use rime_core::{GraphQuantizationConfig, NodeExecutionMode};
use rime_isp::{
    FrameIdentity, FusedColorReproduce, FusedDemosaicThresholds, FusedGamma,
    FusedHighlightRecovery, FusedUniformRequest, LcstStatisticsPacket, ModuleParameterPacket,
    PreparedOperatorMethods, PreprocessContext, build_normal_graph_presentation,
    complete_operator_methods, lcst_producer_by_id, pack_fused_uniforms, prepare_operator_methods,
    vbe::dem::{DEFAULT_AHD_C_THRESHOLD_SQ, DEFAULT_AHD_L_THRESHOLD, DEFAULT_VNG_THRESHOLD},
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameMode {
    Single,
    Sequence,
}

impl FrameMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "single" => Ok(Self::Single),
            "sequence" => Ok(Self::Sequence),
            _ => Err(js_error(
                "WASM_FRAME_MODE_INVALID: expected single or sequence",
            )),
        }
    }
}

struct PendingFrame {
    descriptor: FrameDescriptor,
    options: FrameOptions,
    frame_index: u64,
    mode: FrameMode,
    context: PreprocessContext,
    pre_lcst: PreparedOperatorMethods,
    consumers: Option<PreparedOperatorMethods>,
    current_statistics: Option<LcstStatisticsPacket>,
}

#[wasm_bindgen]
pub struct FramePacketDeriver {
    pending: Option<PendingFrame>,
    history: Option<LcstStatisticsPacket>,
}

impl Default for FramePacketDeriver {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl FramePacketDeriver {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: None,
            history: None,
        }
    }

    /// Prepares the BLC and LCST stages before GPU execution.
    ///
    /// # Errors
    ///
    /// Returns a stable message when a frame is already pending, JSON, metadata,
    /// method selection, or pre-LCST packet derivation fails.
    #[expect(
        clippy::too_many_arguments,
        reason = "the WASM boundary transports the complete typed frame identity without JSON or per-frame wrapper allocation"
    )]
    pub fn begin_frame(
        &mut self,
        descriptor_json: &str,
        raw_samples: &[u16],
        options_json: &str,
        frame_index: u64,
        run_revision: u64,
        method_revision: u64,
        mode: &str,
    ) -> Result<FrameBegin, String> {
        if self.pending.is_some() {
            return Err(js_error(
                "WASM_FRAME_IN_PROGRESS: complete the previous frame first",
            ));
        }
        let mode = FrameMode::parse(mode)?;
        let descriptor: FrameDescriptor = serde_json::from_str(descriptor_json)
            .map_err(|error| js_error(&format!("WASM_DESCRIPTOR_INVALID: {error}")))?;
        let options: FrameOptions = serde_json::from_str(options_json)
            .map_err(|error| js_error(&format!("WASM_FRAME_OPTIONS_INVALID: {error}")))?;
        let expected_samples = usize::try_from(descriptor.row_stride_samples)
            .ok()
            .and_then(|stride| {
                usize::try_from(descriptor.height)
                    .ok()
                    .and_then(|height| stride.checked_mul(height))
            })
            .ok_or_else(|| js_error("WASM_RAW_SAMPLES_INVALID: sample count overflows"))?;
        if raw_samples.len() != expected_samples {
            return Err(js_error(&format!(
                "WASM_RAW_SAMPLES_INVALID: expected {expected_samples} samples, got {}",
                raw_samples.len()
            )));
        }

        let history_is_incompatible = self.history.as_ref().is_some_and(|history| {
            history.source_extent() != [descriptor.width, descriptor.height]
                || history.cfa_pattern() != cfa_pattern(&descriptor.cfa).unwrap_or([u32::MAX; 4])
        });
        if (mode == FrameMode::Sequence && frame_index == 0) || history_is_incompatible {
            self.history = None;
        }

        let context = preprocess_context(
            &descriptor,
            &options,
            raw_samples,
            frame_index,
            run_revision,
            method_revision,
        )?;
        let pre_lcst = prepare_operator_methods(&[("blc", "00")], &context)
            .map_err(|error| js_error(&format!("WASM_PREPROCESS_FAILED: {error}")))?;
        let lcst_packet = lcst_producer_by_id("lcst")
            .ok_or_else(|| js_error("WASM_LCST_METHOD_MISSING: lcst producer is not registered"))?
            .preprocess("00", &context)
            .map_err(|error| js_error(&format!("WASM_LCST_PREPROCESS_FAILED: {error}")))?;
        let begin = FrameBegin {
            blc_uniform: packet(&pre_lcst, "blc")?.bytes().to_vec(),
            lcst_uniform: lcst_packet.bytes().to_vec(),
        };
        self.pending = Some(PendingFrame {
            descriptor,
            options,
            frame_index,
            mode,
            context,
            pre_lcst,
            consumers: None,
            current_statistics: None,
        });
        Ok(begin)
    }

    /// Prepares consumer packets using same-frame statistics for standalone mode
    /// or the committed predecessor/cold-start policy for sequence mode.
    ///
    /// # Errors
    ///
    /// Returns a stable message when statistics are missing, malformed, or the
    /// pending frame is in the wrong lifecycle phase.
    pub fn prepare_consumers(
        &mut self,
        payload: Option<Vec<u8>>,
        sequence_cold_start: bool,
    ) -> Result<FramePackets, String> {
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| js_error("WASM_FRAME_NOT_PREPARED: begin a frame first"))?;
        if pending.consumers.is_some() {
            return Err(js_error(
                "WASM_CONSUMERS_ALREADY_PREPARED: prepare consumers once",
            ));
        }
        let statistics = match pending.mode {
            FrameMode::Single => {
                if sequence_cold_start {
                    return Err(js_error(
                        "WASM_LCST_COLD_START_INVALID: cold start only applies to sequence mode",
                    ));
                }
                let payload = payload.ok_or_else(|| {
                    js_error(
                        "WASM_LCST_PAYLOAD_REQUIRED: standalone consumers require same-frame LCST statistics",
                    )
                })?;
                let statistics = decode_lcst(&pending.context, &payload)?;
                pending.current_statistics = Some(statistics.clone());
                Some(statistics)
            }
            FrameMode::Sequence => {
                if payload.is_some() {
                    return Err(js_error(
                        "WASM_LCST_PAYLOAD_UNEXPECTED: sequence consumers use predecessor statistics",
                    ));
                }
                if sequence_cold_start {
                    if pending.frame_index != 0 {
                        return Err(js_error(
                            "WASM_LCST_COLD_START_INVALID: cold start requires sequence frame zero",
                        ));
                    }
                    None
                } else {
                    if pending.frame_index == 0 {
                        return Err(js_error(
                            "WASM_LCST_COLD_START_REQUIRED: sequence frame zero requires explicit cold start",
                        ));
                    }
                    let history = self.history.as_ref().ok_or_else(|| {
                        js_error(
                            "WASM_LCST_HISTORY_MISSING: sequence predecessor statistics are unavailable",
                        )
                    })?;
                    let expected = pending.frame_index.saturating_sub(1);
                    if history.identity().frame_index != expected
                        || history.identity().method_revision
                            != pending.context.identity.method_revision
                        || history.source_extent()
                            != [pending.context.width, pending.context.height]
                        || history.cfa_pattern() != pending.context.cfa_pattern
                    {
                        return Err(js_error(
                            "WASM_LCST_HISTORY_MISMATCH: sequence predecessor does not match the current frame",
                        ));
                    }
                    Some(history.clone())
                }
            }
        };
        pending.context.lcst_statistics = statistics;
        pending.context.drc_local_cold_start =
            pending.mode == FrameMode::Sequence && sequence_cold_start;
        let selected = [
            ("tintless", "00"),
            ("lsc", "00"),
            ("wbc", "00"),
            ("drc", pending.options.drc_method.as_str()),
            ("dem", pending.options.dem_method.as_str()),
        ];
        let consumers = prepare_operator_methods(&selected, &pending.context)
            .map_err(|error| js_error(&format!("WASM_PREPROCESS_FAILED: {error}")))?;
        let packets = build_frame_packets(
            &pending.descriptor,
            &pending.options,
            pending.frame_index,
            &pending.pre_lcst,
            &consumers,
        )?;
        pending.consumers = Some(consumers);
        Ok(packets)
    }

    /// Decodes and stages the current frame's LCST result for sequence history.
    ///
    /// # Errors
    ///
    /// Returns a stable message when there is no pending frame, the current
    /// payload was already staged, or the payload is malformed.
    pub fn stage_lcst_statistics(&mut self, payload: &[u8]) -> Result<(), String> {
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| js_error("WASM_FRAME_NOT_PREPARED: begin a frame first"))?;
        if pending.current_statistics.is_some() {
            return Err(js_error(
                "WASM_LCST_STATISTICS_ALREADY_STAGED: current frame statistics are already staged",
            ));
        }
        pending.current_statistics = Some(decode_lcst(&pending.context, payload)?);
        Ok(())
    }

    /// Runs the matching registered Rust postprocess hooks and commits sequence history.
    ///
    /// # Errors
    ///
    /// Returns an error when consumer preparation or current-frame LCST staging is missing.
    pub fn complete_frame(&mut self) -> Result<(), String> {
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| js_error("WASM_FRAME_NOT_PREPARED: begin a frame first"))?;
        let consumers = pending.consumers.as_ref().ok_or_else(|| {
            js_error("WASM_CONSUMERS_NOT_PREPARED: prepare consumers before completion")
        })?;
        let statistics = pending.current_statistics.as_ref().ok_or_else(|| {
            js_error(
                "WASM_LCST_PAYLOAD_REQUIRED: stage current-frame LCST statistics before completion",
            )
        })?;
        complete_operator_methods(&pending.pre_lcst)
            .map_err(|error| js_error(&format!("WASM_POSTPROCESS_FAILED: {error}")))?;
        complete_operator_methods(consumers)
            .map_err(|error| js_error(&format!("WASM_POSTPROCESS_FAILED: {error}")))?;
        let mode = pending.mode;
        let statistics = statistics.clone();
        self.pending = None;
        if mode == FrameMode::Sequence {
            self.history = Some(statistics);
        }
        Ok(())
    }

    /// Discards a prepared frame after WebGPU compute failed.
    ///
    /// # Errors
    ///
    /// Returns an error when no frame is pending.
    pub fn abort_frame(&mut self) -> Result<(), String> {
        self.pending
            .take()
            .ok_or_else(|| js_error("WASM_FRAME_NOT_PREPARED: begin a frame first"))?;
        Ok(())
    }

    /// Clears the pending transaction and committed sequence statistics.
    pub fn reset(&mut self) {
        self.pending = None;
        self.history = None;
    }
}

#[derive(Debug)]
#[wasm_bindgen]
pub struct FrameBegin {
    blc_uniform: Vec<u8>,
    lcst_uniform: Vec<u8>,
}

#[wasm_bindgen]
impl FrameBegin {
    #[must_use]
    pub fn blc_uniform(&self) -> Vec<u8> {
        self.blc_uniform.clone()
    }

    #[must_use]
    pub fn lcst_uniform(&self) -> Vec<u8> {
        self.lcst_uniform.clone()
    }
}

fn decode_lcst(
    context: &PreprocessContext,
    payload: &[u8],
) -> Result<LcstStatisticsPacket, String> {
    let producer = lcst_producer_by_id("lcst")
        .ok_or_else(|| js_error("WASM_LCST_METHOD_MISSING: lcst producer is not registered"))?;
    let method = producer
        .method("00")
        .map_err(|error| js_error(&format!("WASM_LCST_METHOD_MISSING: {error}")))?;
    (method.decode)(
        context.identity,
        [context.width, context.height],
        context.cfa_pattern,
        payload,
    )
    .map_err(|error| js_error(&format!("WASM_LCST_DECODE_FAILED: {error}")))
}

#[derive(Debug)]
#[wasm_bindgen]
pub struct FramePackets {
    blc_uniform: Vec<u8>,
    tintless_uniform: Vec<u8>,
    tintless_mesh: Vec<u8>,
    tintless_audit: Vec<u8>,
    lsc_uniform: Vec<u8>,
    lsc_mesh_headers: Vec<u8>,
    lsc_mesh_entries: Vec<u8>,
    lsc_active: bool,
    wbc_uniform: Vec<u8>,
    drc_uniform: Vec<u8>,
    dem_uniform: Vec<u8>,
    drc_global_lut: Vec<u8>,
    drc_local_lut: Vec<u8>,
    drc_modulation_luts: Vec<u8>,
    fused_uniform: Vec<u8>,
    color_reproduce_hs_lut: Vec<u8>,
    wbc_hr_gain: f32,
    preprocess_snapshot: String,
}

#[wasm_bindgen]
impl FramePackets {
    #[must_use]
    pub fn blc_uniform(&self) -> Vec<u8> {
        self.blc_uniform.clone()
    }
    #[must_use]
    pub fn tintless_uniform(&self) -> Vec<u8> {
        self.tintless_uniform.clone()
    }

    #[must_use]
    pub fn tintless_mesh(&self) -> Vec<u8> {
        self.tintless_mesh.clone()
    }

    #[must_use]
    pub fn tintless_audit(&self) -> Vec<u8> {
        self.tintless_audit.clone()
    }

    #[must_use]
    pub fn lsc_uniform(&self) -> Vec<u8> {
        self.lsc_uniform.clone()
    }

    #[must_use]
    pub fn lsc_mesh_headers(&self) -> Vec<u8> {
        self.lsc_mesh_headers.clone()
    }

    #[must_use]
    pub fn lsc_mesh_entries(&self) -> Vec<u8> {
        self.lsc_mesh_entries.clone()
    }

    #[must_use]
    pub fn lsc_active(&self) -> bool {
        self.lsc_active
    }

    #[must_use]
    pub fn wbc_uniform(&self) -> Vec<u8> {
        self.wbc_uniform.clone()
    }

    #[must_use]
    pub fn drc_uniform(&self) -> Vec<u8> {
        self.drc_uniform.clone()
    }

    #[must_use]
    pub fn dem_uniform(&self) -> Vec<u8> {
        self.dem_uniform.clone()
    }

    #[must_use]
    pub fn drc_global_lut(&self) -> Vec<u8> {
        self.drc_global_lut.clone()
    }

    #[must_use]
    pub fn drc_local_lut(&self) -> Vec<u8> {
        self.drc_local_lut.clone()
    }

    #[must_use]
    pub fn drc_modulation_luts(&self) -> Vec<u8> {
        self.drc_modulation_luts.clone()
    }

    #[must_use]
    pub fn fused_uniform(&self) -> Vec<u8> {
        self.fused_uniform.clone()
    }

    #[must_use]
    pub fn color_reproduce_hs_lut(&self) -> Vec<u8> {
        self.color_reproduce_hs_lut.clone()
    }

    #[must_use]
    pub fn wbc_hr_gain(&self) -> f32 {
        self.wbc_hr_gain
    }
    #[must_use]
    pub fn preprocess_snapshot_json(&self) -> String {
        self.preprocess_snapshot.clone()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrameDescriptor {
    width: u32,
    height: u32,
    cfa: String,
    black_level: f32,
    row_stride_samples: u32,
    white_level: f32,
    color_reproduce: Option<ColorReproduceDescriptor>,
    metadata: MetadataDescriptor,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColorReproduceDescriptor {
    sensor_to_prophoto: [f32; 9],
    prophoto_to_srgb: [f32; 9],
    hs_dims: [u32; 2],
    hs_enable: bool,
    #[serde(default)]
    hs_lut: Option<Vec<f32>>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VignetteRadialDescriptor {
    coefficients: [f64; 5],
    optical_center: [f64; 2],
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GainMapMeshDescriptor {
    points: [u32; 2],
    spacing: [f64; 2],
    origin: [f64; 2],
    planes: u32,
    /// `[top, left, bottom, right]`, exclusive bottom/right. Absent or
    /// all-zero means the whole image (DNG `dng_area_spec` semantics;
    /// normalized by the LSC preprocess).
    #[serde(default)]
    area: [i32; 4],
    /// Application grid pitch; anything but 1x1 cannot be reproduced by a
    /// per-pixel mesh and is rejected during preprocessing. Omitted pitch
    /// (the TS contract never sends one) means an unpitched grid.
    #[serde(default = "pitch_one")]
    row_pitch: u32,
    #[serde(default = "pitch_one")]
    col_pitch: u32,
    entries: Vec<f32>,
}

/// Serde default for an omitted pitch: a plain, unpitched grid.
const fn pitch_one() -> u32 {
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDescriptor {
    color_matrix1: [f64; 9],
    #[serde(default)]
    color_matrix2: Option<[f64; 9]>,
    #[serde(default)]
    as_shot_neutral: Option<[f64; 3]>,
    #[serde(default)]
    as_shot_white_xy: Option<[f64; 2]>,
    #[serde(default)]
    camera_calibration1: Option<[f64; 9]>,
    #[serde(default)]
    camera_calibration2: Option<[f64; 9]>,
    #[serde(default)]
    analog_balance: Option<[f64; 3]>,
    #[serde(default)]
    baseline_exposure: Option<f64>,
    #[serde(default)]
    exif_exposure_time: Option<[u32; 2]>,
    #[serde(default)]
    exif_f_number: Option<[u32; 2]>,
    #[serde(default)]
    exif_iso_speed: Option<u16>,
    #[serde(default)]
    gain_maps: Vec<GainMapMeshDescriptor>,
    #[serde(default)]
    exif_brightness_value: Option<f64>,
    #[serde(default)]
    exif_exposure_bias_value: Option<f64>,
    #[serde(default)]
    vignette_radial: Vec<VignetteRadialDescriptor>,
}

#[derive(Deserialize)]
struct FrameOptions {
    dem_method: String,
    drc_method: String,
    drc_gain_offset_ev: Option<f32>,
    drc_knee: Option<f32>,
    drc_amplifier: Option<f32>,
    drc_modulation_curves: Option<ModulationCurvesOptions>,
    drc_details_amplify: bool,
    wbc_highlight_recovery: bool,
    gamma: GammaOptions,
    dem_thresholds: Option<DemosaicThresholdsOptions>,
    quantization: GraphQuantizationConfig,
    #[serde(default)]
    bypass_modules: std::collections::BTreeMap<String, bool>,
}

impl FrameOptions {
    fn is_bypassed(&self, module_id: &str) -> bool {
        self.bypass_modules
            .get(module_id)
            .copied()
            .unwrap_or(module_id == "tintless")
    }
}

#[derive(Deserialize)]
struct GammaOptions {
    gamma: f32,
    lut: [f32; 9],
}

#[derive(Clone, Deserialize)]
struct ModulationCurvesOptions {
    edge: Vec<(f64, f64)>,
    luma: Vec<(f64, f64)>,
}

impl From<&ModulationCurvesOptions> for rime_isp::vbe::drc::DrcModulationCurves {
    fn from(curves: &ModulationCurvesOptions) -> Self {
        Self {
            edge: curves.edge.clone(),
            luma: curves.luma.clone(),
        }
    }
}

/// Demosaic threshold overrides sent by the host. Present = full override;
/// absent = per-method IQ default (VNG constant, AHD scene-brightness LUT).
#[derive(Deserialize)]
struct DemosaicThresholdsOptions {
    vng_threshold: f32,
    ahd_l_threshold: f32,
    ahd_c_threshold_sq: f32,
}

impl From<&DemosaicThresholdsOptions> for rime_isp::DemosaicThresholds {
    fn from(options: &DemosaicThresholdsOptions) -> Self {
        Self {
            vng_threshold: options.vng_threshold,
            ahd_l_threshold: options.ahd_l_threshold,
            ahd_c_threshold_sq: options.ahd_c_threshold_sq,
        }
    }
}

fn preprocess_context(
    descriptor: &FrameDescriptor,
    options: &FrameOptions,
    _raw_samples: &[u16],
    frame_index: u64,
    run_revision: u64,
    method_revision: u64,
) -> Result<PreprocessContext, String> {
    Ok(PreprocessContext {
        identity: FrameIdentity {
            frame_index,
            run_revision,
            method_revision,
        },
        width: descriptor.width,
        height: descriptor.height,
        black_level: descriptor.black_level,
        white_level: descriptor.white_level,
        cfa_pattern: cfa_pattern(&descriptor.cfa)?,
        as_shot_neutral: descriptor.metadata.as_shot_neutral,
        as_shot_white_xy: descriptor.metadata.as_shot_white_xy,
        color_matrix1: descriptor.metadata.color_matrix1,
        color_matrix2: descriptor.metadata.color_matrix2,
        calibration_illuminant1_code: None,
        calibration_illuminant2_code: None,
        camera_calibration1: descriptor.metadata.camera_calibration1,
        camera_calibration2: descriptor.metadata.camera_calibration2,
        camera_calibration_signature: None,
        profile_calibration_signature: None,
        profile_hue_sat_map_dims: None,
        profile_hue_sat_map_data1: None,
        analog_balance: descriptor.metadata.analog_balance,
        profile_hue_sat_map_data2: None,
        scene_brightness_ev: descriptor.metadata.exif_brightness_value.or_else(|| {
            rime_scene::estimate_scene_brightness_ev(&rime_scene::SceneInput {
                aperture_f_number: positive_ratio(descriptor.metadata.exif_f_number),
                exposure_time_seconds: positive_ratio(descriptor.metadata.exif_exposure_time),
                exposure_bias_ev: descriptor.metadata.exif_exposure_bias_value,
                ..rime_scene::SceneInput::default()
            })
            .ok()
        }),
        exposure_deviation_ev: descriptor.metadata.exif_exposure_bias_value,
        iso: descriptor.metadata.exif_iso_speed.map(f64::from),
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: descriptor.metadata.baseline_exposure,
        exposure_time_seconds: positive_ratio(descriptor.metadata.exif_exposure_time),
        f_number: positive_ratio(descriptor.metadata.exif_f_number),
        lcst_statistics: None,
        drc_local_cold_start: false,
        drc_exposure_policy: rime_isp::vbe::drc::DrcExposurePolicy::Baseline,
        drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0,

        drc_gain_offset_ev: options.drc_gain_offset_ev,
        drc_knee: options.drc_knee,
        drc_amplifier: options.drc_amplifier,
        drc_modulation_curves: options.drc_modulation_curves.as_ref().map(Into::into),
        wbc_highlight_recovery: options.wbc_highlight_recovery,
        wbc_hr_gain: None,
        drc_details_amplify: options.drc_details_amplify,
        dem_thresholds: options.dem_thresholds.as_ref().map(Into::into),
        vignette_radial: descriptor
            .metadata
            .vignette_radial
            .iter()
            .map(|vignette| rime_isp::VignetteRadialParameters {
                coefficients: vignette.coefficients,
                optical_center: vignette.optical_center,
            })
            .collect(),
        gain_maps: descriptor
            .metadata
            .gain_maps
            .iter()
            .map(|mesh| rime_isp::GainMapParameters {
                points: mesh.points,
                spacing: mesh.spacing,
                origin: mesh.origin,
                planes: mesh.planes,
                area: mesh.area,
                row_pitch: mesh.row_pitch,
                col_pitch: mesh.col_pitch,
                entries: mesh.entries.clone(),
            })
            .collect(),
    })
}

fn build_frame_packets(
    descriptor: &FrameDescriptor,
    options: &FrameOptions,
    frame_index: u64,
    pre_lcst: &PreparedOperatorMethods,
    consumers: &PreparedOperatorMethods,
) -> Result<FramePackets, String> {
    let blc = packet(pre_lcst, "blc")?;
    let tintless = packet(consumers, "tintless")?;
    let lsc = packet(consumers, "lsc")?;
    let wbc = packet(consumers, "wbc")?;
    let drc = packet(consumers, "drc")?;
    let dem = packet(consumers, "dem")?;
    let wbc_hr_gain = rime_isp::vbe::white_balance::hr_gain_from_packet(wbc)
        .map_err(|error| js_error(&format!("WASM_WBC_PACKET_INVALID: {error}")))?;
    let white_balance_gains = [
        f32_at(wbc.bytes(), 0)?,
        f32_at(wbc.bytes(), 4)?,
        f32_at(wbc.bytes(), 8)?,
    ];
    let demosaic = demosaic_thresholds(options.dem_method.as_str(), dem.bytes())?;
    let color_reproduce = descriptor
        .color_reproduce
        .clone()
        .unwrap_or_else(identity_color_reproduce);
    let presentation = build_normal_graph_presentation();
    options
        .quantization
        .resolve(&presentation)
        .map_err(|error| js_error(&format!("WASM_QUANTIZATION_INVALID: {error}")))?;
    let quantization = options
        .quantization
        .modules
        .iter()
        .map(|module| rime_isp::rime_quant::GpuQuantModuleConfig {
            module_id: module.module_id.clone(),
            output_enabled: module.output_enabled,
            output_profile: module.output_profile.clone(),
            clip_type: module.clip_type,
        })
        .collect();
    let module_modes = presentation
        .nodes
        .iter()
        .filter_map(|node| {
            node.execution_node_id.as_ref().map(|module_id| {
                (
                    module_id.clone(),
                    node.mode == NodeExecutionMode::Enabled && !options.is_bypassed(module_id),
                )
            })
        })
        .collect();
    let fused_frame_index = u32::try_from(frame_index).map_err(|_| {
        js_error("WASM_FRAME_INDEX_INVALID: fused quantization requires a 32-bit frame index")
    })?;
    let fused_uniform = pack_fused_uniforms(&FusedUniformRequest {
        width: descriptor.width,
        height: descriptor.height,
        black_level: descriptor.black_level,
        white_level: descriptor.white_level,
        cfa_pattern: cfa_pattern(&descriptor.cfa)?,
        white_balance_gains,
        demosaic,
        gamma: FusedGamma {
            gamma: options.gamma.gamma,
            lut: options.gamma.lut,
        },
        color_reproduce: FusedColorReproduce {
            sensor_to_prophoto: color_reproduce.sensor_to_prophoto,
            prophoto_to_srgb: color_reproduce.prophoto_to_srgb,
            hs_dims: color_reproduce.hs_dims,
            hs_enable: color_reproduce.hs_enable,
        },
        highlight_recovery: FusedHighlightRecovery {
            enable: options.wbc_highlight_recovery,
            gains: white_balance_gains,
        },
        frame_index: fused_frame_index,
        quantization,
        quantization_graph_enabled: options.quantization.enabled,
        module_modes,
    })
    .map_err(|error| js_error(&error))?;
    let preprocess_snapshot =
        preprocess_snapshot_json(frame_index, options, blc, tintless, wbc, drc, dem)?;
    Ok(FramePackets {
        blc_uniform: blc.bytes().to_vec(),
        tintless_uniform: tintless.bytes().to_vec(),
        tintless_mesh: resource_bytes(tintless, "gain_mesh")?,
        tintless_audit: resource_bytes(tintless, "audit")?,
        lsc_uniform: lsc.bytes().to_vec(),
        lsc_mesh_headers: resource_bytes(lsc, "gain_mesh_headers")?,
        lsc_mesh_entries: resource_bytes(lsc, "gain_mesh_entries")?,
        lsc_active: packet_enabled(lsc),
        wbc_uniform: wbc.bytes().to_vec(),
        drc_uniform: drc.bytes().to_vec(),
        dem_uniform: padded_dem_uniform(dem.bytes()),
        drc_global_lut: resource_bytes(drc, "tone_lut_global")?,
        drc_local_lut: optional_resource_bytes(drc, "tone_lut_local"),
        drc_modulation_luts: resource_bytes(drc, "modulation_luts")?,
        fused_uniform,
        color_reproduce_hs_lut: encode_f32(color_reproduce.hs_lut.as_deref().unwrap_or(&[])),
        wbc_hr_gain,
        preprocess_snapshot,
    })
}

fn packet_enabled(packet: &ModuleParameterPacket) -> bool {
    packet
        .bytes()
        .get(..4)
        .and_then(|slice| slice.try_into().ok())
        .map_or(0_u32, u32::from_ne_bytes)
        != 0
}

fn optional_resource_bytes(packet: &ModuleParameterPacket, id: &str) -> Vec<u8> {
    packet
        .resource(id)
        .map_or_else(Vec::new, |resource| resource.bytes().to_vec())
}
fn resource_bytes(packet: &ModuleParameterPacket, id: &str) -> Result<Vec<u8>, String> {
    packet
        .resource(id)
        .map(|resource| resource.bytes().to_vec())
        .ok_or_else(|| js_error(&format!("WASM_PACKET_RESOURCE_MISSING: {id}")))
}

fn packet<'a>(
    prepared: &'a PreparedOperatorMethods,
    module_id: &str,
) -> Result<&'a ModuleParameterPacket, String> {
    prepared
        .packets()
        .iter()
        .find(|packet| packet.module_id() == module_id)
        .ok_or_else(|| js_error(&format!("WASM_PACKET_MISSING: {module_id}")))
}

fn preprocess_snapshot_json(
    frame_index: u64,
    options: &FrameOptions,
    blc: &ModuleParameterPacket,
    tintless: &ModuleParameterPacket,
    wbc: &ModuleParameterPacket,
    drc: &ModuleParameterPacket,
    dem: &ModuleParameterPacket,
) -> Result<String, String> {
    let mut modules = serde_json::Map::new();
    modules.insert(
        "blc".to_owned(),
        module_snapshot(
            blc,
            &serde_json::json!({
                "black_level": f32_at(blc.bytes(), 0)?,
                "white_level": f32_at(blc.bytes(), 4)?,
                "width": u32_at(blc.bytes(), 8)?,
                "height": u32_at(blc.bytes(), 12)?,
            }),
        ),
    );
    let tintless_audit = tintless
        .resource("audit")
        .ok_or_else(|| js_error("WASM_TINTLESS_PACKET_INVALID: missing audit resource"))?;
    let audit = tintless_audit.bytes();
    let mesh = tintless
        .resource("gain_mesh")
        .ok_or_else(|| js_error("WASM_TINTLESS_PACKET_INVALID: missing gain mesh"))?
        .bytes();
    let mesh_values = mesh
        .chunks_exact(4)
        .map(|bytes| f32::from_ne_bytes(bytes.try_into().expect("four-byte mesh value")))
        .collect::<Vec<_>>();
    if mesh_values.is_empty() || !mesh_values.iter().all(|value| value.is_finite()) {
        return Err(js_error("WASM_TINTLESS_PACKET_INVALID: mesh is not finite"));
    }
    let mesh_min = mesh_values.iter().copied().fold(f32::INFINITY, f32::min);
    let mesh_max = mesh_values
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    modules.insert(
        "tintless".to_owned(),
        module_snapshot(
            tintless,
            &serde_json::json!({
                "source_extent": [u32_at(tintless.bytes(), 0)?, u32_at(tintless.bytes(), 4)?],
                "mesh_extent": [u32_at(tintless.bytes(), 8)?, u32_at(tintless.bytes(), 12)?],
                "cfa_pattern": [u32_at(tintless.bytes(), 16)?, u32_at(tintless.bytes(), 20)?, u32_at(tintless.bytes(), 24)?, u32_at(tintless.bytes(), 28)?],
                "gain_clamp": [f32_at(tintless.bytes(), 32)?, f32_at(tintless.bytes(), 36)?],
                "cold_start": u32_at(tintless.bytes(), 40)? != 0,
                "valid_cells": u32_at(audit, 0)?,
                "qualified_components": u32_at(audit, 4)?,
                "radial_knots": 9,
                "residual_rms": [f32_at(audit, 8)?, f32_at(audit, 12)?],
                "mesh_min": mesh_min,
                "mesh_max": mesh_max,
                "clamp_count": u32_at(audit, 16)?,
            }),
        ),
    );
    modules.insert(
        "wbc".to_owned(),
        module_snapshot(
            wbc,
            &serde_json::json!({
                "red_gain": f32_at(wbc.bytes(), 0)?,
                "green_gain": f32_at(wbc.bytes(), 4)?,
                "blue_gain": f32_at(wbc.bytes(), 8)?,
                "hr_gain": f32_at(wbc.bytes(), 12)?,
                "enable_highlight_recovery": f32_at(wbc.bytes(), 32)? != 0.0,
            }),
        ),
    );
    modules.insert("drc".to_owned(), module_snapshot(drc, &serde_json::json!({
        "drc_gain": f32_at(drc.bytes(), 0)?,
        "hr_gain": f32_at(wbc.bytes(), 12)?,
        "knee": f32_at(drc.bytes(), 4)?,
        "amplifier": f32_at(drc.bytes(), 8)?,
        "enable_details_amplify": (u32_at(drc.bytes(), 28)? & 1) == 1,
        "luma_guard": f32_at(drc.bytes(), 12)?,
        "min_ratio": f32_at(drc.bytes(), 16)?,
        "max_ratio": f32_at(drc.bytes(), 20)?,
        "level_count": u32_at(drc.bytes(), 24)?,
        "feature_flags": u32_at(drc.bytes(), 28)?,
        "analysis_wbc_gains": [
            f32_at(wbc.bytes(), 0)?,
            f32_at(wbc.bytes(), 4)?,
            f32_at(wbc.bytes(), 8)?,
        ],
        "global_tone_lut": format!("{} samples", drc.resource("tone_lut_global").map_or(0, |r| r.bytes().len() / 4)),
        "local_tone_lut": format!("{} bytes", drc.resource("tone_lut_local").map_or(0, |r| r.bytes().len())),
        "modulation_luts": format!("{} samples", drc.resource("modulation_luts").map_or(0, |r| r.bytes().len() / 4)),
    })));
    let mut dem_parameters = serde_json::json!({
        "cfa_pattern": [u32_at(dem.bytes(), 0)?, u32_at(dem.bytes(), 4)?, u32_at(dem.bytes(), 8)?, u32_at(dem.bytes(), 12)?],
    });
    if options.dem_method == "03" {
        dem_parameters["vng_threshold"] = serde_json::json!(f32_at(dem.bytes(), 16)?);
    }
    if options.dem_method == "04" {
        dem_parameters["ahd_l_threshold"] = serde_json::json!(f32_at(dem.bytes(), 20)?);
        dem_parameters["ahd_c_threshold_sq"] = serde_json::json!(f32_at(dem.bytes(), 24)?);
    }
    modules.insert("dem".to_owned(), module_snapshot(dem, &dem_parameters));
    serde_json::to_string(&serde_json::json!({ "frameIndex": frame_index, "modules": modules }))
        .map_err(|error| js_error(&format!("WASM_PREPROCESS_SNAPSHOT_SERIALIZE: {error}")))
}

fn module_snapshot(
    packet: &ModuleParameterPacket,
    parameters: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({ "method": packet.method(), "parameters": parameters })
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| js_error("WASM_PACKET_LAYOUT_INVALID: missing u32 field"))?;
    Ok(u32::from_ne_bytes(
        raw.try_into().expect("validated four-byte packet field"),
    ))
}

fn demosaic_thresholds(method: &str, bytes: &[u8]) -> Result<FusedDemosaicThresholds, String> {
    let mut thresholds = FusedDemosaicThresholds {
        vng_threshold: DEFAULT_VNG_THRESHOLD,
        ahd_l_threshold: DEFAULT_AHD_L_THRESHOLD,
        ahd_c_threshold_sq: DEFAULT_AHD_C_THRESHOLD_SQ,
    };
    if method == "03" {
        thresholds.vng_threshold = f32_at(bytes, 16)?;
    }
    if method == "04" {
        thresholds.ahd_l_threshold = f32_at(bytes, 20)?;
        thresholds.ahd_c_threshold_sq = f32_at(bytes, 24)?;
    }
    Ok(thresholds)
}

fn padded_dem_uniform(bytes: &[u8]) -> Vec<u8> {
    let mut uniform = vec![0_u8; 32];
    let copy_len = bytes.len().min(uniform.len());
    uniform[..copy_len].copy_from_slice(&bytes[..copy_len]);
    uniform
}

fn f32_at(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| js_error("WASM_PACKET_LAYOUT_INVALID: missing f32 field"))?;
    Ok(f32::from_ne_bytes(
        raw.try_into().expect("validated four-byte packet field"),
    ))
}

fn cfa_pattern(cfa: &str) -> Result<[u32; 4], String> {
    match cfa {
        "rggb" => Ok([0, 1, 1, 2]),
        "grbg" => Ok([1, 0, 2, 1]),
        "gbrg" => Ok([1, 2, 0, 1]),
        "bggr" => Ok([2, 1, 1, 0]),
        _ => Err(js_error(&format!("WASM_CFA_INVALID: {cfa}"))),
    }
}

fn positive_ratio(value: Option<[u32; 2]>) -> Option<f64> {
    value.and_then(|[numerator, denominator]| {
        (denominator != 0).then(|| f64::from(numerator) / f64::from(denominator))
    })
}

fn identity_color_reproduce() -> ColorReproduceDescriptor {
    ColorReproduceDescriptor {
        sensor_to_prophoto: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0],
        prophoto_to_srgb: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0],
        hs_dims: [1, 1],
        hs_enable: false,
        hs_lut: None,
    }
}

fn encode_f32(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}

fn js_error(message: &str) -> String {
    message.to_owned()
}
