use rime_core::{GraphQuantizationConfig, NodeExecutionMode};
use rime_isp::{
    FrameIdentity, FusedColorReproduce, FusedDemosaicThresholds, FusedGamma,
    FusedHighlightRecovery, FusedUniformRequest, ModuleParameterPacket, PreparedOperatorMethods,
    PreprocessContext, build_normal_graph_presentation, complete_operator_methods,
    pack_fused_uniforms, prepare_operator_methods,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

const DEFAULT_VNG_THRESHOLD: f32 = 1.5;
const DEFAULT_AHD_L_THRESHOLD: f32 = 2.0;
const DEFAULT_AHD_C_THRESHOLD_SQ: f32 = 4.0;

#[wasm_bindgen]
pub struct FramePacketDeriver {
    prepared: Option<PreparedOperatorMethods>,
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
        Self { prepared: None }
    }

    /// Runs the registered Rust preprocess hooks and returns GPU-ready bytes.
    ///
    /// # Errors
    ///
    /// Returns a stable message when JSON, metadata, method selection, or packet derivation fails.
    pub fn derive_frame_packets(
        &mut self,
        descriptor_json: &str,
        raw_samples: &[u16],
        options_json: &str,
        frame_index: u32,
    ) -> Result<FramePackets, JsValue> {
        if self.prepared.is_some() {
            return Err(js_error(
                "WASM_FRAME_IN_PROGRESS: complete the previous frame first",
            ));
        }
        let descriptor: FrameDescriptor = serde_json::from_str(descriptor_json)
            .map_err(|error| js_error(&format!("WASM_DESCRIPTOR_INVALID: {error}")))?;
        let options: FrameOptions = serde_json::from_str(options_json)
            .map_err(|error| js_error(&format!("WASM_FRAME_OPTIONS_INVALID: {error}")))?;
        let context = preprocess_context(&descriptor, &options, raw_samples, frame_index)?;
        let selected = [
            ("blc", "00"),
            ("wbc", "00"),
            ("drc", options.drc_method.as_str()),
            ("dem", options.dem_method.as_str()),
        ];
        let prepared = prepare_operator_methods(&selected, &context)
            .map_err(|error| js_error(&format!("WASM_PREPROCESS_FAILED: {error}")))?;
        let packets = build_frame_packets(&descriptor, &options, frame_index, &prepared)?;
        self.prepared = Some(prepared);
        Ok(packets)
    }

    /// Runs the matching registered Rust postprocess hooks after WebGPU compute.
    ///
    /// # Errors
    ///
    /// Returns an error when no frame is pending or a postprocess hook fails.
    pub fn complete_frame(&mut self) -> Result<(), JsValue> {
        let prepared = self
            .prepared
            .take()
            .ok_or_else(|| js_error("WASM_FRAME_NOT_PREPARED: derive frame packets first"))?;
        complete_operator_methods(&prepared)
            .map_err(|error| js_error(&format!("WASM_POSTPROCESS_FAILED: {error}")))?;
        Ok(())
    }

    /// Discards a prepared frame after WebGPU compute failed.
    ///
    /// # Errors
    ///
    /// Returns an error when no frame is pending.
    pub fn abort_frame(&mut self) -> Result<(), JsValue> {
        self.prepared
            .take()
            .ok_or_else(|| js_error("WASM_FRAME_NOT_PREPARED: derive frame packets first"))?;
        Ok(())
    }
}

#[wasm_bindgen]
pub struct FramePackets {
    blc_uniform: Vec<u8>,
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
    exif_brightness_value: Option<f64>,
    #[serde(default)]
    exif_exposure_bias_value: Option<f64>,
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
    quantization: GraphQuantizationConfig,
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

fn preprocess_context(
    descriptor: &FrameDescriptor,
    options: &FrameOptions,
    raw_samples: &[u16],
    frame_index: u32,
) -> Result<PreprocessContext, JsValue> {
    let local_statistics = if options.drc_method == "01" {
        Some(
            rime_isp::vbe::drc::build_bayer_local_statistics(
                raw_samples,
                descriptor.width,
                descriptor.height,
                descriptor.row_stride_samples,
                descriptor.black_level,
                descriptor.white_level,
            )
            .map_err(|error| js_error(&format!("WASM_DRC_HISTOGRAM_INVALID: {error}")))?,
        )
    } else {
        None
    };
    Ok(PreprocessContext {
        identity: FrameIdentity {
            frame_index: u64::from(frame_index),
            run_revision: 0,
            method_revision: 0,
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
        profile_hue_sat_map_data2: None,
        analog_balance: descriptor.metadata.analog_balance,
        scene_brightness_ev: descriptor.metadata.exif_brightness_value,
        exposure_deviation_ev: descriptor.metadata.exif_exposure_bias_value,
        iso: descriptor.metadata.exif_iso_speed.map(f64::from),
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: descriptor.metadata.baseline_exposure,
        exposure_time_seconds: positive_ratio(descriptor.metadata.exif_exposure_time),
        f_number: positive_ratio(descriptor.metadata.exif_f_number),
        drc_local_statistics: local_statistics,
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
    })
}

fn build_frame_packets(
    descriptor: &FrameDescriptor,
    options: &FrameOptions,
    frame_index: u32,
    prepared: &PreparedOperatorMethods,
) -> Result<FramePackets, JsValue> {
    let blc = packet(prepared, "blc")?;
    let wbc = packet(prepared, "wbc")?;
    let drc = packet(prepared, "drc")?;
    let dem = packet(prepared, "dem")?;
    let wbc_hr_gain = rime_isp::vfe::white_balance::hr_gain_from_packet(wbc)
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
            node.execution_node_id
                .as_ref()
                .map(|module_id| (module_id.clone(), node.mode == NodeExecutionMode::Enabled))
        })
        .collect();
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
        frame_index,
        quantization,
        quantization_graph_enabled: options.quantization.enabled,
        module_modes,
    })
    .map_err(|error| js_error(&error))?;
    let preprocess_snapshot = preprocess_snapshot_json(frame_index, options, blc, wbc, drc, dem)?;
    Ok(FramePackets {
        blc_uniform: blc.bytes().to_vec(),
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

fn packet<'a>(
    prepared: &'a PreparedOperatorMethods,
    module_id: &str,
) -> Result<&'a ModuleParameterPacket, JsValue> {
    prepared
        .packets()
        .iter()
        .find(|packet| packet.module_id() == module_id)
        .ok_or_else(|| js_error(&format!("WASM_PACKET_MISSING: {module_id}")))
}

fn resource_bytes(packet: &ModuleParameterPacket, id: &str) -> Result<Vec<u8>, JsValue> {
    packet
        .resource(id)
        .map(|resource| resource.bytes().to_vec())
        .ok_or_else(|| js_error(&format!("WASM_PACKET_RESOURCE_MISSING: {id}")))
}

fn optional_resource_bytes(packet: &ModuleParameterPacket, id: &str) -> Vec<u8> {
    packet
        .resource(id)
        .map_or_else(Vec::new, |resource| resource.bytes().to_vec())
}

fn preprocess_snapshot_json(
    frame_index: u32,
    options: &FrameOptions,
    blc: &ModuleParameterPacket,
    wbc: &ModuleParameterPacket,
    drc: &ModuleParameterPacket,
    dem: &ModuleParameterPacket,
) -> Result<String, JsValue> {
    let mut modules = serde_json::Map::new();
    modules.insert("blc".to_owned(), module_snapshot(blc, &serde_json::json!({
        "black_level": f32_at(blc.bytes(), 0)?,
        "white_level": f32_at(blc.bytes(), 4)?,
        "width": u32_at(blc.bytes(), 8)?,
        "height": u32_at(blc.bytes(), 12)?,
    })));
    modules.insert("wbc".to_owned(), module_snapshot(wbc, &serde_json::json!({
        "red_gain": f32_at(wbc.bytes(), 0)?,
        "green_gain": f32_at(wbc.bytes(), 4)?,
        "blue_gain": f32_at(wbc.bytes(), 8)?,
        "hr_gain": f32_at(wbc.bytes(), 12)?,
        "enable_highlight_recovery": f32_at(wbc.bytes(), 32)? != 0.0,
    })));
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
        dem_parameters["vng_threshold"] = serde_json::json!(DEFAULT_VNG_THRESHOLD);
    }
    if options.dem_method == "04" {
        dem_parameters["ahd_l_threshold"] = serde_json::json!(f32_at(dem.bytes(), 20)?);
        dem_parameters["ahd_c_threshold_sq"] = serde_json::json!(f32_at(dem.bytes(), 24)?);
    }
    modules.insert("dem".to_owned(), module_snapshot(dem, &dem_parameters));
    serde_json::to_string(&serde_json::json!({ "frameIndex": frame_index, "modules": modules }))
        .map_err(|error| js_error(&format!("WASM_PREPROCESS_SNAPSHOT_SERIALIZE: {error}")))
}

fn module_snapshot(packet: &ModuleParameterPacket, parameters: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "method": packet.method(), "parameters": parameters })
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, JsValue> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| js_error("WASM_PACKET_LAYOUT_INVALID: missing u32 field"))?;
    Ok(u32::from_ne_bytes(
        raw.try_into().expect("validated four-byte packet field"),
    ))
}

fn demosaic_thresholds(method: &str, bytes: &[u8]) -> Result<FusedDemosaicThresholds, JsValue> {
    let mut thresholds = FusedDemosaicThresholds {
        vng_threshold: DEFAULT_VNG_THRESHOLD,
        ahd_l_threshold: DEFAULT_AHD_L_THRESHOLD,
        ahd_c_threshold_sq: DEFAULT_AHD_C_THRESHOLD_SQ,
    };
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

fn f32_at(bytes: &[u8], offset: usize) -> Result<f32, JsValue> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| js_error("WASM_PACKET_LAYOUT_INVALID: missing f32 field"))?;
    Ok(f32::from_ne_bytes(
        raw.try_into().expect("validated four-byte packet field"),
    ))
}

fn cfa_pattern(cfa: &str) -> Result<[u32; 4], JsValue> {
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

fn js_error(message: &str) -> JsValue {
    JsValue::from_str(message)
}
