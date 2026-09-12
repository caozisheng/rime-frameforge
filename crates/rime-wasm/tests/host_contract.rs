use rime_wasm::{FramePacketDeriver, NormalRuntime};

#[test]
fn wasm_runtime_exposes_the_normal_manifest() {
    let runtime = NormalRuntime::new();

    assert!(runtime.manifest_json().contains("\"graph_id\":\"normal\""));
}

#[test]
fn wasm_runtime_step_exposes_warmup_snapshot() {
    let mut runtime = NormalRuntime::new();
    runtime.load().expect("built-in manifest must load");

    let snapshot = runtime.step().expect("step may start");

    assert!(snapshot.contains("\"frame_phase\":\"warmup\""));
}

#[test]
fn wasm_runtime_failure_enters_error() {
    let mut runtime = NormalRuntime::new();
    runtime.load().expect("built-in manifest must load");

    let snapshot = runtime.fail();

    assert!(snapshot.contains("\"lifecycle_state\":\"error\""));
}

#[test]
fn wasm_runtime_accepts_quantization_config_and_increments_revision() {
    let mut runtime = NormalRuntime::new();
    runtime.load().expect("built-in manifest must load");
    let graph = rime_isp::build_normal_graph_presentation();
    let mut config = rime_core::GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.enabled = false;
    let snapshot = runtime
        .set_quantization_config(&serde_json::to_string(&config).expect("config JSON"))
        .expect("valid config");
    assert!(snapshot.contains("\"config_revision\":1"));
}

#[test]
fn wasm_runtime_exposes_generic_config_revision_change() {
    let mut runtime = NormalRuntime::new();
    runtime.load().expect("built-in manifest must load");
    let snapshot = runtime
        .change_config()
        .expect("config change may start from stop");
    assert!(snapshot.contains("\"config_revision\":1"));
}

fn frame_descriptor() -> serde_json::Value {
    serde_json::json!({
        "width": 4,
        "height": 3,
        "rowStrideSamples": 4,
        "cfa": "rggb",
        "blackLevel": 64.0,
        "whiteLevel": 4095.0,
        "colorReproduce": {
            "sensorToProphoto": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0],
            "prophotoToSrgb": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0],
            "hsDims": [1, 1],
            "hsEnable": false,
            "hsLut": null
        },
        "metadata": {
            "colorMatrix1": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0],
            "colorMatrix2": null,
            "asShotNeutral": [0.5, 1.0, 0.75],
            "asShotWhiteXy": null,
            "cameraCalibration1": null,
            "cameraCalibration2": null,
            "analogBalance": null,
            "baselineExposure": 0.0,
            "exifExposureTime": null,
            "exifFNumber": null,
            "exifIsoSpeed": null,
            "exifBrightnessValue": null,
            "exifExposureBiasValue": null
        }
    })
}

fn direct_context() -> rime_isp::PreprocessContext {
    rime_isp::PreprocessContext {
        identity: rime_isp::FrameIdentity {
            frame_index: 11,
            run_revision: 0,
            method_revision: 0,
        },
        width: 4,
        height: 3,
        black_level: 64.0,
        white_level: 4095.0,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: Some([0.5, 1.0, 0.75]),
        as_shot_white_xy: None,
        color_matrix1: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0],
        color_matrix2: None,
        calibration_illuminant1_code: None,
        calibration_illuminant2_code: None,
        camera_calibration1: None,
        camera_calibration2: None,
        camera_calibration_signature: None,
        profile_calibration_signature: None,
        profile_hue_sat_map_dims: None,
        profile_hue_sat_map_data1: None,
        profile_hue_sat_map_data2: None,
        analog_balance: None,
        scene_brightness_ev: None,
        exposure_deviation_ev: None,
        iso: None,
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: Some(0.0),
        exposure_time_seconds: None,
        f_number: None,
        drc_local_statistics: None,
        drc_exposure_policy: rime_isp::vbe::drc::DrcExposurePolicy::Baseline,
        drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0,
        drc_gain_offset_ev: Some(0.0),
        drc_knee: Some(1.0),
        drc_amplifier: Some(3.0),
        drc_modulation_curves: None,
        wbc_highlight_recovery: true,
        wbc_hr_gain: None,
        drc_details_amplify: true,
    }
}

#[test]
fn wasm_deriver_returns_rust_preprocess_packets() {
    let graph = rime_isp::build_normal_graph_presentation();
    let quantization =
        rime_core::GraphQuantizationConfig::defaults_for(&graph).expect("quantization defaults");
    let descriptor = frame_descriptor();
    let options = serde_json::json!({
        "dem_method": "00",
        "drc_method": "00",
        "drc_gain_offset_ev": 0.0,
        "drc_knee": 1.0,
        "drc_amplifier": 3.0,
        "drc_details_amplify": true,
        "wbc_highlight_recovery": true,
        "gamma": {
            "gamma": 2.2,
            "lut": [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0]
        },
        "quantization": quantization
    });
    let raw_samples = [64_u16; 12];
    let mut deriver = FramePacketDeriver::new();
    let packets = deriver
        .derive_frame_packets(
            &descriptor.to_string(),
            &raw_samples,
            &options.to_string(),
            11,
        )
        .expect("valid frame packets");
    let direct_context = direct_context();
    let direct = rime_isp::prepare_operator_methods(
        &[("blc", "00"), ("wbc", "00"), ("drc", "00"), ("dem", "00")],
        &direct_context,
    )
    .expect("direct Rust preprocess packets");
    let packet = |module_id| {
        direct
            .packets()
            .iter()
            .find(|packet| packet.module_id() == module_id)
            .expect("selected packet")
    };
    let drc = packet("drc");

    assert_eq!(packets.blc_uniform(), packet("blc").bytes());
    assert_eq!(packets.wbc_uniform(), packet("wbc").bytes());
    assert_eq!(packets.drc_uniform(), drc.bytes());
    assert_eq!(packets.dem_uniform(), packet("dem").bytes());
    assert_eq!(
        packets.drc_global_lut(),
        drc.resource("tone_lut_global")
            .expect("global tone LUT")
            .bytes()
    );
    assert_eq!(
        packets.drc_modulation_luts(),
        drc.resource("modulation_luts")
            .expect("modulation LUTs")
            .bytes()
    );
    assert!(drc.resource("tone_lut_local").is_none());

    assert_eq!(packets.blc_uniform().len(), 16);
    assert_eq!(packets.wbc_uniform().len(), 48);
    assert_eq!(packets.drc_uniform().len(), 32);
    assert_eq!(packets.dem_uniform().len(), 32);
    assert_eq!(packets.drc_global_lut().len(), 257 * 4);
    assert_eq!(packets.drc_modulation_luts().len(), 128 * 4);
    assert!(packets.drc_local_lut().is_empty());
    assert_eq!(packets.fused_uniform().len(), rime_isp::FUSED_UNIFORM_BYTES);
    assert!(packets.color_reproduce_hs_lut().is_empty());
    deriver.complete_frame().expect("postprocess hooks");
}

#[test]
fn wasm_deriver_can_abort_a_failed_gpu_frame() {
    let graph = rime_isp::build_normal_graph_presentation();
    let quantization =
        rime_core::GraphQuantizationConfig::defaults_for(&graph).expect("quantization defaults");
    let options = serde_json::json!({
        "dem_method": "00",
        "drc_method": "00",
        "drc_gain_offset_ev": 0.0,
        "drc_knee": 1.0,
        "drc_amplifier": 3.0,
        "drc_details_amplify": true,
        "wbc_highlight_recovery": false,
        "gamma": {
            "gamma": 2.2,
            "lut": [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0]
        },
        "quantization": quantization
    });
    let descriptor = frame_descriptor().to_string();
    let options = options.to_string();
    let raw_samples = [64_u16; 12];
    let mut deriver = FramePacketDeriver::new();

    deriver
        .derive_frame_packets(&descriptor, &raw_samples, &options, 11)
        .expect("first frame preprocess");
    deriver.abort_frame().expect("failed GPU frame abort");
    deriver
        .derive_frame_packets(&descriptor, &raw_samples, &options, 12)
        .expect("next frame preprocess");
    deriver.complete_frame().expect("next frame postprocess");
}
