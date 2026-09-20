#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "fixed LCST fixture grids have bounded coordinates and exact packet bit checks"
)]

use rime_wasm::{FramePacketDeriver, FramePackets, NormalRuntime};

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
        "width": 128,
        "height": 96,
        "rowStrideSamples": 128,
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

fn lifecycle_options(enable_lcst: bool) -> String {
    let graph = rime_isp::build_normal_graph_presentation();
    let quantization =
        rime_core::GraphQuantizationConfig::defaults_for(&graph).expect("quantization defaults");
    serde_json::json!({
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
        "quantization": quantization,
        "bypass_modules": {"tintless": !enable_lcst}
    })
    .to_string()
}
fn lifecycle_options_with_drc(method: &str) -> String {
    let mut options: serde_json::Value =
        serde_json::from_str(&lifecycle_options(true)).expect("lifecycle options JSON");
    options["drc_method"] = serde_json::json!(method);
    options.to_string()
}

fn lcst_payload(average: f32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(rime_isp::LCST_PAYLOAD_BYTES);
    for _ in 0..rime_isp::LCST_AVERAGE_VALUES {
        payload.extend_from_slice(&average.to_ne_bytes());
    }
    for tile_y in 0..rime_isp::LCST_HISTOGRAM_GRID_HEIGHT {
        let y0 = (tile_y as u32) * 96 / 16;
        let y1 = ((tile_y as u32) + 1) * 96 / 16;
        for tile_x in 0..rime_isp::LCST_HISTOGRAM_GRID_WIDTH {
            let x0 = (tile_x as u32) * 128 / 16;
            let x1 = ((tile_x as u32) + 1) * 128 / 16;
            let count = (x1 - x0) * (y1 - y0);
            payload.extend_from_slice(&count.to_ne_bytes());
            for _ in 1..rime_isp::LCST_HISTOGRAM_BINS {
                payload.extend_from_slice(&0_u32.to_ne_bytes());
            }
        }
    }
    assert_eq!(payload.len(), rime_isp::LCST_PAYLOAD_BYTES);
    payload
}

fn lcst_payload_variant(radial_r: f32, radial_b: f32, histogram_bin: usize) -> Vec<u8> {
    assert!(histogram_bin < rime_isp::LCST_HISTOGRAM_BINS);
    let mut payload = Vec::with_capacity(rime_isp::LCST_PAYLOAD_BYTES);
    let corner = (63.5_f32.mul_add(63.5, 47.5_f32 * 47.5)).sqrt();
    for y in 0..rime_isp::LCST_AVERAGE_GRID_HEIGHT {
        for x in 0..rime_isp::LCST_AVERAGE_GRID_WIDTH {
            let dx = x as f32 - 31.5;
            let dy = y as f32 - 23.5;
            let radius = dx.mul_add(dx, dy * dy).sqrt() / corner;
            for value in [
                0.25 * (radial_r * radius).exp(),
                0.25,
                0.25,
                0.25 * (radial_b * radius).exp(),
            ] {
                payload.extend_from_slice(&value.to_ne_bytes());
            }
        }
    }
    for tile_y in 0..rime_isp::LCST_HISTOGRAM_GRID_HEIGHT {
        let y0 = (tile_y as u32) * 96 / 16;
        let y1 = ((tile_y as u32) + 1) * 96 / 16;
        for tile_x in 0..rime_isp::LCST_HISTOGRAM_GRID_WIDTH {
            let x0 = (tile_x as u32) * 128 / 16;
            let x1 = ((tile_x as u32) + 1) * 128 / 16;
            let count = (x1 - x0) * (y1 - y0);
            let next_bin = (histogram_bin + 1).min(rime_isp::LCST_HISTOGRAM_BINS - 1);
            let first_count = count / 2;
            for bin in 0..rime_isp::LCST_HISTOGRAM_BINS {
                let value = if bin == histogram_bin {
                    first_count
                } else if bin == next_bin {
                    count - first_count
                } else {
                    0
                };
                payload.extend_from_slice(&value.to_ne_bytes());
            }
        }
    }
    assert_eq!(payload.len(), rime_isp::LCST_PAYLOAD_BYTES);
    payload
}

fn derive_single_packets_with_payload(
    descriptor: &str,
    raw_samples: &[u16],
    options: &str,
    frame_index: u64,
    payload: Vec<u8>,
) -> FramePackets {
    let mut deriver = FramePacketDeriver::new();
    deriver
        .begin_frame(
            descriptor,
            raw_samples,
            options,
            frame_index,
            0,
            0,
            "single",
        )
        .expect("frame begin");
    let packets = deriver
        .prepare_consumers(Some(payload), false)
        .expect("same-frame LCST consumers");
    deriver.complete_frame().expect("frame completion");
    packets
}

fn derive_single_packets(
    descriptor: &str,
    raw_samples: &[u16],
    options: &str,
    frame_index: u32,
) -> FramePackets {
    derive_single_packets_with_payload(
        descriptor,
        raw_samples,
        options,
        u64::from(frame_index),
        lcst_payload(0.25),
    )
}

#[test]
fn staged_frames_reject_a_second_begin_before_completion() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(false);
    let raw_samples = vec![64_u16; 128 * 96];
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "single")
        .expect("first frame begin");
    let error = deriver
        .begin_frame(&descriptor, &raw_samples, &options, 1, 0, 0, "single")
        .expect_err("second frame must be rejected while pending");
    assert_eq!(
        error,
        "WASM_FRAME_IN_PROGRESS: complete the previous frame first"
    );
    deriver.abort_frame().expect("abort pending frame");
}

#[test]
fn standalone_consumers_require_same_frame_lcst_bytes() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "single")
        .expect("frame begin");
    let error = deriver
        .prepare_consumers(None, false)
        .expect_err("standalone consumers require LCST bytes");
    assert_eq!(
        error,
        "WASM_LCST_PAYLOAD_REQUIRED: standalone consumers require same-frame LCST statistics"
    );
    deriver.abort_frame().expect("abort pending frame");
}

#[test]
fn standalone_lcst_bytes_feed_consumers_and_stage_history() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let payload = lcst_payload(0.25);
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "single")
        .expect("frame begin");
    let packets = deriver
        .prepare_consumers(Some(payload), false)
        .expect("same-frame LCST decode");
    assert!(packets.tintless_mesh().iter().any(|byte| *byte != 0));
    deriver.complete_frame().expect("complete frame");
}
#[test]
fn standalone_lcst_bytes_feed_drc01_local_tone_mapping() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options_with_drc("01");
    let raw_samples = vec![64_u16; 128 * 96];
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "single")
        .expect("frame begin");
    let packets = deriver
        .prepare_consumers(Some(lcst_payload(0.25)), false)
        .expect("DRC01 same-frame LCST consumers");

    assert_eq!(
        packets.drc_local_lut().len(),
        257 * 16 * 16 * std::mem::size_of::<f32>()
    );
    deriver.complete_frame().expect("complete frame");
}

#[test]
fn distinct_lcst_statistics_change_tintless_and_drc01_packets() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options_with_drc("01");
    let raw_samples = vec![64_u16; 128 * 96];

    let low_scene = derive_single_packets_with_payload(
        &descriptor,
        &raw_samples,
        &options,
        0,
        lcst_payload_variant(0.30, -0.20, 2),
    );
    let high_scene = derive_single_packets_with_payload(
        &descriptor,
        &raw_samples,
        &options,
        0,
        lcst_payload_variant(-0.20, 0.25, 12),
    );

    assert_ne!(low_scene.tintless_mesh(), high_scene.tintless_mesh());
    assert_ne!(low_scene.drc_local_lut(), high_scene.drc_local_lut());
}

#[test]
fn standalone_lcst_decode_rejects_malformed_payloads() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let mut truncated = lcst_payload(0.25);
    truncated.pop();
    let mut non_finite = lcst_payload(0.25);
    non_finite[..std::mem::size_of::<f32>()].copy_from_slice(&f32::NAN.to_ne_bytes());
    let mut bad_histogram = lcst_payload(0.25);
    bad_histogram[rime_isp::LCST_AVERAGE_BYTES..rime_isp::LCST_AVERAGE_BYTES + 4]
        .copy_from_slice(&0_u32.to_ne_bytes());

    for (payload, expected) in [
        (
            truncated,
            "WASM_LCST_DECODE_FAILED: LCST payload byte length is invalid",
        ),
        (
            non_finite,
            "WASM_LCST_DECODE_FAILED: LCST average payload contains a non-finite value",
        ),
        (
            bad_histogram,
            "WASM_LCST_DECODE_FAILED: LCST histogram tile total does not match its source partition",
        ),
    ] {
        let mut deriver = FramePacketDeriver::new();
        deriver
            .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "single")
            .expect("frame begin");
        assert_eq!(
            deriver
                .prepare_consumers(Some(payload), false)
                .expect_err("malformed LCST payload must fail"),
            expected
        );
        deriver.abort_frame().expect("abort malformed frame");
    }
}

#[test]
fn sequence_history_requires_explicit_cold_start_and_commits_only_on_complete() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let payload = lcst_payload(0.25);
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "sequence")
        .expect("sequence frame zero begin");
    let cold = deriver
        .prepare_consumers(None, true)
        .expect("explicit sequence cold start");
    assert!(
        cold.tintless_mesh()
            .chunks_exact(std::mem::size_of::<f32>())
            .all(|bytes| f32::from_ne_bytes(bytes.try_into().expect("f32 mesh value")) == 1.0)
    );
    deriver
        .stage_lcst_statistics(&payload)
        .expect("stage current frame statistics");
    deriver
        .abort_frame()
        .expect("aborted frame must not publish history");

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 1, 0, 0, "sequence")
        .expect("next sequence frame begin");
    let error = deriver
        .prepare_consumers(None, false)
        .expect_err("aborted frame must not publish predecessor history");
    assert_eq!(
        error,
        "WASM_LCST_HISTORY_MISSING: sequence predecessor statistics are unavailable"
    );
    deriver.abort_frame().expect("abort frame without history");

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "sequence")
        .expect("restart sequence frame zero");
    deriver
        .prepare_consumers(None, true)
        .expect("cold start after restart");
    deriver
        .stage_lcst_statistics(&payload)
        .expect("stage frame zero statistics");
    deriver.complete_frame().expect("commit frame zero history");

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 1, 1, 0, "sequence")
        .expect("sequence frame one begin under the next runtime run revision");
    let warm = deriver
        .prepare_consumers(None, false)
        .expect("predecessor history across per-frame run revisions");
    assert!(warm.tintless_mesh().iter().any(|byte| *byte != 0));
    deriver
        .stage_lcst_statistics(&lcst_payload(0.30))
        .expect("stage frame one statistics");
    deriver.complete_frame().expect("commit frame one history");
}

#[test]
fn sequence_frame_zero_always_requires_explicit_cold_start() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "sequence")
        .expect("initial sequence frame zero");
    deriver
        .prepare_consumers(None, true)
        .expect("initial cold start");
    deriver
        .stage_lcst_statistics(&lcst_payload(0.25))
        .expect("stage frame zero statistics");
    deriver.complete_frame().expect("commit frame zero history");

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 0, 0, "sequence")
        .expect("restarted sequence frame zero");
    let error = deriver
        .prepare_consumers(None, false)
        .expect_err("frame zero must not consume stale frame-zero history");
    assert_eq!(
        error,
        "WASM_LCST_COLD_START_REQUIRED: sequence frame zero requires explicit cold start"
    );
    deriver.abort_frame().expect("abort restarted frame zero");
}
#[test]
fn sequence_history_is_invalidated_by_extent_or_cfa_changes() {
    let descriptor = frame_descriptor();
    let descriptor_json = descriptor.to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let payload = lcst_payload(0.25);

    for changed_descriptor in [
        {
            let mut changed = descriptor.clone();
            changed["cfa"] = serde_json::json!("bggr");
            changed
        },
        {
            let mut changed = descriptor.clone();
            changed["width"] = serde_json::json!(256);
            changed["rowStrideSamples"] = serde_json::json!(256);
            changed
        },
    ] {
        let mut deriver = FramePacketDeriver::new();
        deriver
            .begin_frame(
                &descriptor_json,
                &raw_samples,
                &options,
                0,
                0,
                0,
                "sequence",
            )
            .expect("sequence frame zero begin");
        deriver
            .prepare_consumers(None, true)
            .expect("sequence cold start");
        deriver
            .stage_lcst_statistics(&payload)
            .expect("stage frame zero statistics");
        deriver.complete_frame().expect("commit frame zero history");

        let changed_raw_samples =
            vec![
                64_u16;
                usize::try_from(changed_descriptor["rowStrideSamples"].as_u64().unwrap()).unwrap()
                    * usize::try_from(changed_descriptor["height"].as_u64().unwrap()).unwrap()
            ];
        deriver
            .begin_frame(
                &changed_descriptor.to_string(),
                &changed_raw_samples,
                &options,
                1,
                0,
                0,
                "sequence",
            )
            .expect("changed sequence frame begin");
        assert_eq!(
            deriver
                .prepare_consumers(None, false)
                .expect_err("incompatible history must not be consumed"),
            "WASM_LCST_HISTORY_MISSING: sequence predecessor statistics are unavailable"
        );
        deriver.abort_frame().expect("abort changed frame");
    }
}

#[test]
fn sequence_history_allows_next_run_revision_but_rejects_method_revision_changes() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let payload = lcst_payload(0.25);
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 7, 11, "sequence")
        .expect("sequence frame zero begin");
    deriver
        .prepare_consumers(None, true)
        .expect("sequence frame zero cold start");
    deriver
        .stage_lcst_statistics(&payload)
        .expect("stage frame zero statistics");
    deriver.complete_frame().expect("commit frame zero history");

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 1, 8, 11, "sequence")
        .expect("changed-run frame begin");
    deriver
        .prepare_consumers(None, false)
        .expect("per-frame runtime run revisions preserve predecessor history");
    deriver.abort_frame().expect("abort changed-run frame");

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 1, 7, 12, "sequence")
        .expect("changed-method frame begin");
    assert_eq!(
        deriver
            .prepare_consumers(None, false)
            .expect_err("history from another method revision must be rejected"),
        "WASM_LCST_HISTORY_MISMATCH: sequence predecessor does not match the current frame"
    );
    deriver.abort_frame().expect("abort changed-method frame");
}

#[test]
fn resetting_the_deriver_clears_pending_state_and_sequence_history() {
    let descriptor = frame_descriptor().to_string();
    let options = lifecycle_options(true);
    let raw_samples = vec![64_u16; 128 * 96];
    let mut deriver = FramePacketDeriver::new();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 0, 7, 11, "sequence")
        .expect("sequence frame zero begin");
    deriver
        .prepare_consumers(None, true)
        .expect("sequence cold start");
    deriver
        .stage_lcst_statistics(&lcst_payload(0.25))
        .expect("stage frame zero statistics");
    deriver.complete_frame().expect("commit frame zero history");

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 1, 7, 11, "sequence")
        .expect("sequence frame one begin");
    deriver.reset();

    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 1, 7, 11, "sequence")
        .expect("frame begin after reset");
    assert_eq!(
        deriver
            .prepare_consumers(None, false)
            .expect_err("reset must clear predecessor history"),
        "WASM_LCST_HISTORY_MISSING: sequence predecessor statistics are unavailable"
    );
    deriver.abort_frame().expect("abort reset frame");
}

fn direct_context() -> rime_isp::PreprocessContext {
    rime_isp::PreprocessContext {
        identity: rime_isp::FrameIdentity {
            frame_index: 11,
            run_revision: 0,
            method_revision: 0,
        },
        width: 128,
        height: 96,
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
        lcst_statistics: None,
        drc_local_cold_start: false,
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
        dem_thresholds: None,
        vignette_radial: Vec::new(),
        gain_maps: Vec::new(),
    }
}

fn assert_preprocess_snapshot(packets: &FramePackets) {
    let snapshot: serde_json::Value = serde_json::from_str(&packets.preprocess_snapshot_json())
        .expect("valid preprocess snapshot JSON");
    assert_eq!(snapshot["frameIndex"], 11);
    assert_eq!(snapshot["modules"]["wbc"]["parameters"]["red_gain"], 2.0);
    assert_eq!(
        snapshot["modules"]["drc"]["parameters"]["luma_guard"],
        1.0 / 65_536.0
    );
    assert_eq!(snapshot["modules"]["dem"]["method"], "00");
    assert_eq!(
        snapshot["modules"]["dem"]["parameters"]["cfa_pattern"],
        serde_json::json!([0, 1, 1, 2])
    );
    assert!(
        snapshot["modules"]["dem"]["parameters"]
            .get("ahd_l_threshold")
            .is_none()
    );
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
    let raw_samples = vec![64_u16; 128 * 96];
    let packets = derive_single_packets(
        &descriptor.to_string(),
        &raw_samples,
        &options.to_string(),
        11,
    );
    let direct_context = direct_context();
    let direct = rime_isp::prepare_operator_methods(
        &[
            ("blc", "00"),
            ("lsc", "00"),
            ("wbc", "00"),
            ("drc", "00"),
            ("dem", "00"),
        ],
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
    assert_eq!(packets.lsc_uniform(), packet("lsc").bytes());
    assert_eq!(
        packets.lsc_mesh_headers(),
        packet("lsc")
            .resource("gain_mesh_headers")
            .expect("LSC mesh headers")
            .bytes()
    );
    assert_eq!(
        packets.lsc_mesh_entries(),
        packet("lsc")
            .resource("gain_mesh_entries")
            .expect("LSC mesh entries")
            .bytes()
    );
    assert!(!packets.lsc_active());
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
    assert_preprocess_snapshot(&packets);
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
    let raw_samples = vec![64_u16; 128 * 96];
    let mut deriver = FramePacketDeriver::new();
    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 11, 0, 0, "single")
        .expect("first frame preprocess");
    deriver.abort_frame().expect("failed GPU frame abort");
    deriver
        .begin_frame(&descriptor, &raw_samples, &options, 12, 0, 0, "single")
        .expect("next frame preprocess");
    deriver
        .prepare_consumers(Some(lcst_payload(0.25)), false)
        .expect("next frame consumers");
    deriver.complete_frame().expect("next frame postprocess");
}

#[test]
fn wasm_deriver_applies_dem_threshold_overrides() {
    let graph = rime_isp::build_normal_graph_presentation();
    let quantization =
        rime_core::GraphQuantizationConfig::defaults_for(&graph).expect("quantization defaults");
    let mut descriptor = frame_descriptor();
    descriptor["metadata"]["exifBrightnessValue"] = serde_json::json!(0.0);
    let base_options = serde_json::json!({
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
    let raw_samples = vec![64_u16; 128 * 96];
    let f32_at = |bytes: &[u8], offset: usize| {
        f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap())
    };

    // dem03 (VNG): override lands at uniform offset 16; absent override
    // keeps the module default.
    let mut with_override = base_options.clone();
    with_override["dem_method"] = serde_json::json!("03");
    with_override["dem_thresholds"] = serde_json::json!({
        "vng_threshold": 3.25,
        "ahd_l_threshold": 2.0,
        "ahd_c_threshold_sq": 4.0
    });
    let packets = derive_single_packets(
        &descriptor.to_string(),
        &raw_samples,
        &with_override.to_string(),
        11,
    );
    assert_eq!(f32_at(&packets.dem_uniform(), 16), 3.25);
    let snapshot: serde_json::Value =
        serde_json::from_str(&packets.preprocess_snapshot_json()).expect("snapshot JSON");
    assert_eq!(
        snapshot["modules"]["dem"]["parameters"]["vng_threshold"],
        3.25
    );

    let mut without_override = base_options.clone();
    without_override["dem_method"] = serde_json::json!("03");
    let packets = derive_single_packets(
        &descriptor.to_string(),
        &raw_samples,
        &without_override.to_string(),
        11,
    );
    assert_eq!(f32_at(&packets.dem_uniform(), 16), 1.5);

    let mut ahd_override = base_options.clone();
    ahd_override["dem_method"] = serde_json::json!("04");
    ahd_override["dem_thresholds"] = serde_json::json!({
        "vng_threshold": 1.5,
        "ahd_l_threshold": 2.0,
        "ahd_c_threshold_sq": 4.0
    });
    let packets = derive_single_packets(
        &descriptor.to_string(),
        &raw_samples,
        &ahd_override.to_string(),
        11,
    );
    assert_eq!(f32_at(&packets.dem_uniform(), 20), 2.0);
    assert_eq!(f32_at(&packets.dem_uniform(), 24), 4.0);

    let mut ahd_default = base_options;
    ahd_default["dem_method"] = serde_json::json!("04");
    let packets = derive_single_packets(
        &descriptor.to_string(),
        &raw_samples,
        &ahd_default.to_string(),
        11,
    );
    assert_eq!(f32_at(&packets.dem_uniform(), 20), 1.05);
    assert_eq!(f32_at(&packets.dem_uniform(), 24), 3.15);
}
