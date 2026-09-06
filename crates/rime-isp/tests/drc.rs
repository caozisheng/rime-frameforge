#![expect(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "DRC tests compare exact LUT endpoints and exactly representable powers of two"
)]

use rime_isp::vbe::drc::{
    DrcExposureInputs, DrcExposurePolicy, DrcExposureSource, resolve_drc_exposure,
};

fn inputs() -> DrcExposureInputs {
    DrcExposureInputs {
        baseline_exposure_ev: None,
        exposure_bias_ev: None,
        brightness_value: None,
        exposure_time_seconds: None,
        f_number: None,
        iso: None,
        metered_target_ev100: None,
        profile_adjustment_ev: 0.0,
    }
}

#[test]
fn baseline_policy_converts_positive_ev_to_linear_gain() {
    let resolved = resolve_drc_exposure(
        &DrcExposureInputs {
            baseline_exposure_ev: Some(1.5),
            ..inputs()
        },
        DrcExposurePolicy::Baseline,
    )
    .expect("valid baseline exposure");

    assert!((resolved.lift_ev - 1.5).abs() < 1e-12);
    assert!((resolved.drc_gain - 2.0_f64.powf(1.5)).abs() < 1e-12);
    assert_eq!(resolved.source, DrcExposureSource::BaselineExposure);
}

#[test]
fn baseline_policy_does_not_brighten_negative_offsets() {
    let resolved = resolve_drc_exposure(
        &DrcExposureInputs {
            baseline_exposure_ev: Some(-0.75),
            ..inputs()
        },
        DrcExposurePolicy::Baseline,
    )
    .expect("valid baseline exposure");

    assert_eq!(resolved.lift_ev, 0.0);
    assert_eq!(resolved.drc_gain, 1.0);
}

#[test]
fn additive_policy_applies_negative_capture_bias_once() {
    let resolved = resolve_drc_exposure(
        &DrcExposureInputs {
            baseline_exposure_ev: Some(0.5),
            exposure_bias_ev: Some(-0.75),
            ..inputs()
        },
        DrcExposurePolicy::BaselinePlusCaptureBias,
    )
    .expect("profile declares additive semantics");

    assert!((resolved.lift_ev - 1.25).abs() < 1e-12);
    assert_eq!(resolved.source, DrcExposureSource::BaselinePlusCaptureBias);
}

#[test]
fn capture_bias_is_not_used_without_explicit_policy() {
    let fallback = resolve_drc_exposure(
        &DrcExposureInputs {
            exposure_bias_ev: Some(-1.0),
            ..inputs()
        },
        DrcExposurePolicy::Baseline,
    )
    .expect("missing baseline uses the safe fallback");
    assert_eq!(fallback.drc_gain, 1.0);
    assert_eq!(fallback.source, DrcExposureSource::Fallback);

    let capture_only = resolve_drc_exposure(
        &DrcExposureInputs {
            exposure_bias_ev: Some(-1.0),
            ..inputs()
        },
        DrcExposurePolicy::CaptureBiasOnly,
    )
    .expect("capture-only policy is explicit");
    assert_eq!(capture_only.drc_gain, 2.0);
    assert_eq!(capture_only.source, DrcExposureSource::CaptureBias);
}

#[test]
fn calibrated_metering_uses_capture_settings_and_profile_target() {
    let exposure = 1.0 / 100.0;
    let f_number = 2.8;
    let capture_ev100 = f64::log2(f_number * f_number / exposure);
    let resolved = resolve_drc_exposure(
        &DrcExposureInputs {
            brightness_value: Some(8.0),
            exposure_time_seconds: Some(exposure),
            f_number: Some(f_number),
            iso: Some(100.0),
            metered_target_ev100: Some(10.0),
            profile_adjustment_ev: 0.5,
            ..inputs()
        },
        DrcExposurePolicy::CalibratedMetering,
    )
    .expect("calibrated target is complete");

    let expected_lift = (10.0 - capture_ev100 + 0.5).max(0.0);
    assert!((resolved.capture_ev100.expect("capture EV") - capture_ev100).abs() < 1e-12);
    assert!((resolved.lift_ev - expected_lift).abs() < 1e-12);
    assert_eq!(resolved.source, DrcExposureSource::CalibratedMetering);
}

#[test]
fn non_finite_exposure_metadata_is_rejected() {
    let error = resolve_drc_exposure(
        &DrcExposureInputs {
            baseline_exposure_ev: Some(f64::NAN),
            ..inputs()
        },
        DrcExposurePolicy::Baseline,
    )
    .expect_err("NaN metadata must not reach the LUT generator");

    assert_eq!(error.to_string(), "non-finite DRC exposure metadata");
}

#[test]
fn global_tone_lut_is_identity_at_unity_gain() {
    let lut = rime_isp::vbe::drc::generate_global_tone_lut(1.0, 1.0, 257).expect("unity curve");

    for (index, value) in lut.values().iter().copied().enumerate() {
        let expected = index as f32 / 256.0;
        assert!((value - expected).abs() < 1e-5, "sample {index}");
    }
}

#[test]
fn global_tone_lut_brightens_midtones_and_remains_monotonic() {
    let lut =
        rime_isp::vbe::drc::generate_global_tone_lut(2.0, 1.0, 257).expect("two-stop-domain curve");

    assert!(lut.sample(0.5) > 0.5);
    assert_eq!(lut.sample(-1.0), 0.0);
    assert_eq!(lut.sample(2.0), 1.0);
    assert!(lut.values().windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn invalid_global_tone_parameters_are_rejected() {
    let error = rime_isp::vbe::drc::generate_global_tone_lut(0.0, 1.0, 257)
        .expect_err("zero gain is invalid");
    assert_eq!(error.to_string(), "invalid DRC tone parameter");
}

#[test]
fn local_tone_lut_falls_back_for_empty_tiles_and_stays_monotonic() {
    use rime_isp::vbe::drc::{DrcLocalStatistics, LocalToneConfig};

    let global =
        rime_isp::vbe::drc::generate_global_tone_lut(2.0, 1.0, 17).expect("global fallback");
    let stats = DrcLocalStatistics::new(
        2,
        1,
        8,
        vec![32, 24, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    )
    .expect("two tile histograms");
    let local = rime_isp::vbe::drc::generate_local_tone_lut(
        &stats,
        &global,
        LocalToneConfig {
            local_strength: 1.0,
            spatial_smoothing_passes: 1,
            minimum_samples: 16,
        },
    )
    .expect("local LUT field");

    assert_eq!(local.tiles_x(), 2);
    assert!(
        local
            .tile_values(0)
            .windows(2)
            .all(|pair| pair[0] <= pair[1])
    );
    assert_eq!(local.tile_values(1), global.values());
}

#[test]
fn module_parameter_packet_owns_frozen_lut_resources() {
    use rime_isp::{FrameIdentity, ModuleParameterPacket, ModuleParameterResource};

    let identity = FrameIdentity {
        frame_index: 4,
        run_revision: 2,
        method_revision: 3,
    };
    let mut packet =
        ModuleParameterPacket::new("drc", "00", identity, &[0; 16]).expect("small uniform");
    packet
        .push_resource(ModuleParameterResource::new(
            "tone_lut",
            [257, 1, 1],
            vec![1, 2, 3, 4],
        ))
        .expect("unique resource");

    let resource = packet.resource("tone_lut").expect("frozen tone LUT");
    assert_eq!(resource.extent(), [257, 1, 1]);
    assert_eq!(resource.bytes(), &[1, 2, 3, 4]);
}

#[test]
fn local_tone_lut_falls_back_to_global_for_flat_tiles() {
    use rime_isp::vbe::drc::{DrcLocalStatistics, LocalToneConfig};

    let global =
        rime_isp::vbe::drc::generate_global_tone_lut(2.0, 1.0, 17).expect("global fallback");
    let statistics =
        DrcLocalStatistics::new(1, 1, 8, vec![0, 0, 0, 64, 0, 0, 0, 0]).expect("flat tile");
    let local = rime_isp::vbe::drc::generate_local_tone_lut(
        &statistics,
        &global,
        LocalToneConfig {
            local_strength: 1.0,
            spatial_smoothing_passes: 1,
            minimum_samples: 16,
        },
    )
    .expect("local LUT");

    assert_eq!(local.tile_values(0), global.values());
}

#[test]
fn drc00_preprocess_resolves_baseline_and_freezes_global_lut() {
    use rime_isp::{FrameIdentity, Operator as _, PreprocessContext};

    let context = PreprocessContext {
        identity: FrameIdentity {
            frame_index: 8,
            run_revision: 2,
            method_revision: 5,
        },
        width: 64,
        height: 48,
        black_level: 0.0,
        white_level: 4095.0,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: Some([0.5, 1.0, 0.25]),
        as_shot_white_xy: None,
        color_matrix1: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        color_matrix2: None,
        analog_balance: None,
        scene_brightness_ev: None,
        exposure_deviation_ev: None,
        iso: Some(100.0),
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: Some(1.0),
        exposure_time_seconds: Some(0.01),
        f_number: Some(2.8),
        drc_local_statistics: None,
        drc_exposure_policy: DrcExposurePolicy::Baseline,
        drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0,
    };

    let packet = rime_isp::vbe::drc::OPERATOR
        .preprocess("00", &context)
        .expect("DRC00 preprocessing");
    let gain = f32::from_ne_bytes(packet.bytes()[0..4].try_into().expect("gain bytes"));
    assert_eq!(gain, 2.0);
    let cfa_gains = (0..4)
        .map(|index| {
            f32::from_ne_bytes(
                packet.bytes()[32 + index * 4..36 + index * 4]
                    .try_into()
                    .expect("CFA gain bytes"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(cfa_gains, [2.0, 1.0, 1.0, 4.0]);

    let mut additive = context.clone();
    additive.exposure_deviation_ev = Some(-0.5);
    additive.drc_exposure_policy = DrcExposurePolicy::BaselinePlusCaptureBias;
    let additive_packet = rime_isp::vbe::drc::OPERATOR
        .preprocess("00", &additive)
        .expect("additive DRC policy");
    let additive_gain = f32::from_ne_bytes(
        additive_packet.bytes()[0..4]
            .try_into()
            .expect("additive gain bytes"),
    );
    assert!((additive_gain - 2.0_f32.powf(1.5)).abs() < 1e-6);

    let mut missing_white_balance = context.clone();
    missing_white_balance.as_shot_neutral = None;
    missing_white_balance.as_shot_white_xy = None;
    let fallback_packet = rime_isp::vbe::drc::OPERATOR
        .preprocess("00", &missing_white_balance)
        .expect("missing WBC normalization uses identity guide gains");
    let fallback_gains = (0..4)
        .map(|index| {
            f32::from_ne_bytes(
                fallback_packet.bytes()[32 + index * 4..36 + index * 4]
                    .try_into()
                    .expect("fallback CFA gain bytes"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(fallback_gains, [1.0; 4]);
    let lut = packet.resource("tone_lut_global").expect("global tone LUT");
    assert_eq!(lut.extent(), [257, 1, 1]);
    assert_eq!(lut.bytes().len(), 257 * size_of::<f32>());
}

#[test]
fn drc01_preprocess_freezes_local_lut_field() {
    use rime_isp::vbe::drc::DrcLocalStatistics;
    use rime_isp::{FrameIdentity, Operator as _, PreprocessContext};

    let context = PreprocessContext {
        identity: FrameIdentity {
            frame_index: 9,
            run_revision: 2,
            method_revision: 6,
        },
        width: 8,
        height: 4,
        black_level: 0.0,
        white_level: 1.0,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: Some([1.0, 1.0, 1.0]),
        as_shot_white_xy: None,
        color_matrix1: [1.0; 9],
        color_matrix2: None,
        analog_balance: None,
        scene_brightness_ev: None,
        exposure_deviation_ev: None,
        iso: Some(100.0),
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: Some(1.0),
        exposure_time_seconds: Some(0.01),
        f_number: Some(2.8),
        drc_local_statistics: Some(
            DrcLocalStatistics::new(2, 1, 4, vec![8, 4, 2, 2, 2, 2, 4, 8]).expect("local stats"),
        ),
        drc_exposure_policy: DrcExposurePolicy::Baseline,
        drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0,
    };

    let packet = rime_isp::vbe::drc::OPERATOR
        .preprocess("01", &context)
        .expect("DRC01 preprocessing");
    assert!(packet.resource("tone_lut_global").is_some());
    let local = packet
        .resource("tone_lut_local")
        .expect("local tone LUT field");
    assert_eq!(local.extent(), [257, 2, 1]);
    assert_eq!(local.bytes().len(), 257 * 2 * size_of::<f32>());

    let mut missing_statistics = context;
    missing_statistics.drc_local_statistics = None;
    let error = rime_isp::vbe::drc::OPERATOR
        .preprocess("01", &missing_statistics)
        .expect_err("DRC01 must not fabricate local statistics");
    assert!(
        error
            .to_string()
            .contains("requires frozen local histogram statistics")
    );
}

#[test]
fn drc_gpu_plan_declares_pyramid_guided_filter_and_tone_passes() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    for entry in [
        "drc_prefilter_main",
        "pyramid_downsample_main",
        "pyramid_reconstruct_main",
        "guided_stats_horizontal_main",
        "guided_stats_vertical_main",
        "guided_coefficients_main",
        "guided_coefficients_horizontal_main",
        "guided_apply_vertical_main",
        "drc_combine_global_main",
        "drc_combine_local_main",
    ] {
        assert!(shader.contains(entry), "missing DRC pass {entry}");
    }
    assert!(
        shader.contains("local_grid"),
        "local LUT lookup must interpolate tiles"
    );
    assert!(
        shader.contains("edge_mask"),
        "guided base must protect strong edges"
    );
    assert!(
        shader.contains("highlight_protection"),
        "local tone must protect shadows and highlights"
    );
}
