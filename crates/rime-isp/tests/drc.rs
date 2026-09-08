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
        drc_gain_offset_ev: None,
        drc_knee: None,
        drc_amplifier: None,
        wbc_highlight_recovery: false,
    };

    let packet = rime_isp::vbe::drc::OPERATOR
        .preprocess("00", &context)
        .expect("DRC00 preprocessing");
    let gain = f32::from_ne_bytes(packet.bytes()[0..4].try_into().expect("gain bytes"));
    assert_eq!(gain, 2.0);
    let amplifier = f32::from_ne_bytes(packet.bytes()[8..12].try_into().expect("amplifier bytes"));
    assert_eq!(amplifier, 3.0, "Sony A7 reference uses amplifier=3.0");
    assert_eq!(packet.bytes().len(), 32);
    let mut different_white_balance = context.clone();
    different_white_balance.as_shot_neutral = Some([1.0, 0.5, 2.0]);
    let adjusted_packet = rime_isp::vbe::drc::OPERATOR
        .preprocess("00", &different_white_balance)
        .expect("DRC consumes the already balanced VFE output");
    assert_eq!(
        packet.bytes(),
        adjusted_packet.bytes(),
        "WBC now runs upstream in VFE; DRC must not re-apply white-balance gains"
    );

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
        drc_gain_offset_ev: None,
        drc_knee: None,
        drc_amplifier: None,
        wbc_highlight_recovery: false,
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
fn drc_iq_offset_scales_metadata_gain_and_overrides_scalars() {
    use rime_isp::{FrameIdentity, Operator as _, PreprocessContext};

    let context = PreprocessContext {
        identity: FrameIdentity { frame_index: 10, run_revision: 1, method_revision: 1 },
        width: 1,
        height: 1,
        black_level: 0.0,
        white_level: 4095.0,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: Some([1.0, 1.0, 1.0]),
        as_shot_white_xy: None,
        color_matrix1: [1.0; 9],
        color_matrix2: None,
        analog_balance: None,
        scene_brightness_ev: None,
        exposure_deviation_ev: None,
        iso: None,
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: Some(1.0),
        exposure_time_seconds: None,
        f_number: None,
        drc_local_statistics: None,
        drc_exposure_policy: DrcExposurePolicy::Baseline,
        drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0,
        drc_gain_offset_ev: Some(1.0),
        drc_knee: Some(0.5),
        drc_amplifier: Some(2.5),
         wbc_highlight_recovery: false,
    };
    let packet = rime_isp::vbe::drc::OPERATOR.preprocess("00", &context).expect("DRC00");
    let scalar = |offset| f32::from_ne_bytes(packet.bytes()[offset..offset + 4].try_into().unwrap());
    assert_eq!(scalar(0), 4.0);
    assert_eq!(scalar(4), 0.5);
    assert_eq!(scalar(8), 2.5);
    assert_eq!(scalar(20), 16.0);
    let default_packet = rime_isp::vbe::drc::OPERATOR
        .preprocess(
            "00",
            &PreprocessContext {
                drc_gain_offset_ev: None,
                drc_knee: None,
                drc_amplifier: None,
                ..context.clone()
            },
        )
        .expect("default DRC IQ values");
    let default_gain = f32::from_ne_bytes(default_packet.bytes()[0..4].try_into().unwrap());
    assert_eq!(default_gain, 2.0);

    let half_gain_packet = rime_isp::vbe::drc::OPERATOR
        .preprocess(
            "00",
            &PreprocessContext {
                drc_gain_offset_ev: Some(-1.0),
                ..context
            },
        )
        .expect("negative DRC IQ offset");
    let half_gain = f32::from_ne_bytes(half_gain_packet.bytes()[0..4].try_into().unwrap());
    assert_eq!(half_gain, 1.0);
}

#[test]
fn drc_iq_rejects_non_finite_and_out_of_range_values() {
    use rime_isp::{FrameIdentity, Operator as _, PreprocessContext};

    let base = PreprocessContext {
        identity: FrameIdentity { frame_index: 11, run_revision: 1, method_revision: 1 },
        width: 1, height: 1, black_level: 0.0, white_level: 4095.0,
        cfa_pattern: [0, 1, 1, 2], as_shot_neutral: Some([1.0, 1.0, 1.0]), as_shot_white_xy: None,
        color_matrix1: [1.0; 9], color_matrix2: None, analog_balance: None,
        scene_brightness_ev: None, exposure_deviation_ev: None, iso: None,
        analog_gain: None, digital_gain: None, baseline_exposure_ev: Some(1.0),
        exposure_time_seconds: None, f_number: None, drc_local_statistics: None,
        drc_exposure_policy: DrcExposurePolicy::Baseline, drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0, drc_gain_offset_ev: None, drc_knee: None,
        drc_amplifier: None,
        wbc_highlight_recovery: false,
    };
    for (name, context) in [
        ("offset", PreprocessContext { drc_gain_offset_ev: Some(f32::NAN), ..base.clone() }),
        ("knee", PreprocessContext { drc_knee: Some(1.1), ..base.clone() }),
        ("amplifier", PreprocessContext { drc_amplifier: Some(-1.0), ..base.clone() }),
    ] {
        let error = rime_isp::vbe::drc::OPERATOR.preprocess("00", &context).expect_err(name);
        assert!(error.to_string().contains("DRC IQ"), "{name}: {error}");
    }
}

#[test]
fn drc_gpu_plan_declares_pyramid_guided_filter_and_tone_passes() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    for entry in [
        "drc_prefilter_main",
        "pyramid_downsample_main",
        "pyramid_reconstruct_main",
        "guided_coefficients_main",
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
    assert!(
        shader.contains("sum += load_zero(input_a, position).x"),
        "DRC luma must be the direct 3x3 Bayer convolution"
    );
    assert!(!shader.contains("block_luma"));
    assert!(!shader.contains("block_origin"));
}

#[test]
fn drc_uses_gradient_guided_filter_coefficients() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    assert!(shader.contains("gradient_guided"));
    assert!(shader.contains("gradient_chi"));
    assert!(shader.contains("GRADIENT_GUIDED_RADIUS_1"));
    assert!(shader.contains("gradient_weight"));
}

#[test]
fn drc_gradient_guided_coefficients_preserve_regularized_covariance() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    assert!(
        shader.contains("variance + regularization") || shader.contains("variance + epsilon"),
        "gradient-guided a denominator must use eps/weight"
    );
    assert!(
        shader.contains("variance + regularization") && shader.contains("regularization"),
        "gradient-guided a numerator must retain covariance plus regularized detail"
    );
}

#[test]
fn drc_guided_statistics_come_from_the_shared_filter_component() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    let shared = rime_isp::primitives::guided_filter::GUIDED_FILTER_SHARED_FUNCTIONS;
    assert!(
        shader.starts_with(shared),
        "DRC must splice the shared gf_* statistics instead of duplicating them"
    );
    assert!(
        shared.contains("gf_box_moments") && shared.contains("sum_sq += value * value;"),
        "second moments must accumulate in f32 registers before variance subtraction"
    );
    assert!(
        shader.contains("gf_box_moments(input_a, position, vec2<i32>(size))"),
        "the coefficients pass must consume the shared box-moment helper"
    );
    assert!(
        !shader.contains("guided_stats_horizontal_main"),
        "separable stats textures double DRC transient memory and exceed iGPU budgets"
    );
}

#[test]
fn drc00_tone_maps_analysis_luma_before_detail_recovery() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    assert!(
        shader.contains("combine(id.xy, lookup_global(luma))"),
        "reference DRC00 maps y0, then restores protected detail"
    );
}

#[test]
fn drc_edge_modulation_uses_imgradient_sobel_scale() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    assert!(
        shader.contains("return sqrt(gx * gx + gy * gy) / 8.0;"),
        "MATLAB imgradient Sobel magnitude is normalized before edge curve lookup"
    );
}
#[test]
fn rgb_highlights_clip_at_one_shared_saturation_point() {
    let colored = rime_isp::primitives::shared_saturation_clip([2.0, 1.0, 0.5]);
    assert_eq!(colored, [1.0, 0.5, 0.25]);
    let near_neutral = rime_isp::primitives::shared_saturation_clip([2.0, 1.6, 1.8]);
    assert_eq!(near_neutral, [1.0, 1.0, 1.0]);
    let in_range = rime_isp::primitives::shared_saturation_clip([0.8, 0.4, 0.2]);
    assert_eq!(in_range, [0.8, 0.4, 0.2]);
}

#[test]
fn drc_output_clamps_to_normalized_saturation_when_not_quantized() {
    let shader = rime_isp::vbe::drc::DRC_PIPELINE_WGSL;
    assert!(
        shader.contains("clamp(raw * clamp(target_value / luma, params.min_ratio, params.max_ratio), 0.0, 1.0)"),
        "DRC combine must clamp the gain-mapped Bayer to the normalized [0, 1] output domain"
    );
}
