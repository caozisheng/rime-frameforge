#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "validated exposure gains and fixed LUT extents are narrowed to the GPU f32/u32 contract"
)]

use crate::operator::{
    ModuleParameterPacket, ModuleParameterResource, OperatorError, PreprocessContext,
};

use super::{
    DrcExposureInputs, LocalToneConfig, generate_global_tone_lut, generate_local_tone_lut,
    resolve_drc_exposure,
};

const TONE_SAMPLES: usize = 257;
const MODULATION_SAMPLES: usize = 64;
const DEFAULT_KNEE: f32 = 1.0;
const DEFAULT_AMPLIFIER: f32 = 3.0;
const MAX_GAIN_OFFSET_EV: f32 = 4.0;
const DEFAULT_MIN_RATIO: f32 = 1.0 / 256.0;
const DEFAULT_LUMA_GUARD: f32 = 1.0 / 65_536.0;
const DEFAULT_LEVEL_COUNT: u32 = 3;

const REFERENCE_EDGE_CURVE: [(f64, f64); 9] = [
    (0.0, 1.0),
    (0.1, 0.9),
    (0.2, 0.7),
    (0.3, 0.5),
    (0.4, 0.3),
    (0.5, 0.2),
    (0.8, 0.0),
    (1.0, 0.0),
    (10.0, 0.0),
];
const REFERENCE_LUMA_CURVE: [(f64, f64); 6] = [
    (0.0, 0.0),
    (0.2, 0.3),
    (0.3, 0.6),
    (0.5, 0.7),
    (0.7, 0.8),
    (1.0, 1.0),
];

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    prepare(context, module_id, method, false)
}

pub(crate) fn run_local(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    prepare(context, module_id, method, true)
}

fn prepare(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
    local: bool,
) -> Result<ModuleParameterPacket, OperatorError> {
    let drc_gain = resolve_gain(context, module_id)?;
    let knee = resolve_knee(context, module_id)?;
    let amplifier = resolve_amplifier(context, module_id)?;
    let global = generate_global_tone_lut(drc_gain, knee, TONE_SAMPLES).map_err(|_| {
        OperatorError::Preprocess {
            module_id,
            reason: "failed to generate DRC tone LUT",
        }
    })?;
    let local_field = resolve_local_tone(context, &global, local, module_id)?;
    let tiles_x = local_field
        .as_ref()
        .map_or(0, super::LocalToneLutField::tiles_x);
    let tiles_y = local_field
        .as_ref()
        .map_or(0, super::LocalToneLutField::tiles_y);
    let mut uniform = [0_u8; 32];
    write_f32(&mut uniform, 0, drc_gain);
    write_f32(&mut uniform, 4, knee);
    write_f32(&mut uniform, 8, amplifier);
    write_f32(&mut uniform, 12, DEFAULT_LUMA_GUARD);
    write_f32(&mut uniform, 16, DEFAULT_MIN_RATIO);
    write_f32(&mut uniform, 20, drc_gain * 4.0);
    write_u32(&mut uniform, 24, DEFAULT_LEVEL_COUNT);
    let feature_flags = 1 | ((tiles_x & 0xff) << 8) | ((tiles_y & 0xff) << 16);
    write_u32(&mut uniform, 28, feature_flags);
    let modulation_luts = bake_modulation_luts();
    let mut packet = ModuleParameterPacket::new(module_id, method, context.identity, &uniform)?;
    packet.push_resource(ModuleParameterResource::new(
        "tone_lut_global",
        [TONE_SAMPLES as u32, 1, 1],
        encode_f32(global.values()),
    ))?;
    packet.push_resource(ModuleParameterResource::new(
        "modulation_luts",
        [MODULATION_SAMPLES as u32, 2, 1],
        encode_f32(&modulation_luts),
    ))?;
    if let Some(local_field) = local_field {
        packet.push_resource(ModuleParameterResource::new(
            "tone_lut_local",
            [
                TONE_SAMPLES as u32,
                local_field.tiles_x(),
                local_field.tiles_y(),
            ],
            encode_f32(local_field.values()),
        ))?;
    }
    Ok(packet)
}

fn bake_modulation_luts() -> Vec<f32> {
    let mut luts = Vec::with_capacity(MODULATION_SAMPLES * 2);
    let scale = 1.0_f64 / (MODULATION_SAMPLES as f64 - 1.0);
    for sample in 0..MODULATION_SAMPLES {
        luts.push(interpolate_reference_curve(&REFERENCE_EDGE_CURVE, sample as f64 * scale) as f32);
    }
    for sample in 0..MODULATION_SAMPLES {
        luts.push(interpolate_reference_curve(&REFERENCE_LUMA_CURVE, sample as f64 * scale) as f32);
    }
    luts
}

fn interpolate_reference_curve(curve: &[(f64, f64)], x: f64) -> f64 {
    if x <= curve[0].0 {
        return curve[0].1;
    }
    for window in curve.windows(2) {
        let (x0, y0) = window[0];
        let (x1, y1) = window[1];
        if x < x1 {
            let t = (x - x0) / (x1 - x0);
            return y0 + t * (y1 - y0);
        }
    }
    curve[curve.len() - 1].1
}
fn resolve_gain(
    context: &PreprocessContext,
    module_id: &'static str,
) -> Result<f32, OperatorError> {
    let offset = context.drc_gain_offset_ev.unwrap_or(0.0);
    if !offset.is_finite() || !(-MAX_GAIN_OFFSET_EV..=MAX_GAIN_OFFSET_EV).contains(&offset) {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "invalid DRC IQ gain offset",
        });
    }
    let exposure = resolve_drc_exposure(
        &DrcExposureInputs {
            baseline_exposure_ev: context.baseline_exposure_ev,
            exposure_bias_ev: context.exposure_deviation_ev,
            brightness_value: context.scene_brightness_ev,
            exposure_time_seconds: context.exposure_time_seconds,
            f_number: context.f_number,
            iso: context.iso,
            metered_target_ev100: context.drc_metered_target_ev100,
            profile_adjustment_ev: context.drc_profile_adjustment_ev,
        },
        context.drc_exposure_policy,
    )
    .map_err(|_| OperatorError::Preprocess {
        module_id,
        reason: "invalid DRC exposure metadata",
    })?;
    let drc_gain = ((exposure.drc_gain as f32) * 2.0_f32.powf(offset)).max(1.0);
    if !drc_gain.is_finite() || drc_gain <= 0.0 {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "DRC gain is outside the GPU f32 domain",
        });
    }
    Ok(drc_gain)
}

fn resolve_knee(
    context: &PreprocessContext,
    module_id: &'static str,
) -> Result<f32, OperatorError> {
    let knee = context.drc_knee.unwrap_or(DEFAULT_KNEE);
    if !knee.is_finite() || !(0.0..=1.0).contains(&knee) {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "invalid DRC IQ knee",
        });
    }
    Ok(knee)
}

fn resolve_amplifier(
    context: &PreprocessContext,
    module_id: &'static str,
) -> Result<f32, OperatorError> {
    let amplifier = context.drc_amplifier.unwrap_or(DEFAULT_AMPLIFIER);
    if !amplifier.is_finite() || amplifier < 0.0 {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "invalid DRC IQ amplifier",
        });
    }
    Ok(amplifier)
}

fn resolve_local_tone(
    context: &PreprocessContext,
    global: &super::ToneLut,
    local: bool,
    module_id: &'static str,
) -> Result<Option<super::LocalToneLutField>, OperatorError> {
    if !local {
        return Ok(None);
    }
    let statistics = context
        .drc_local_statistics
        .as_ref()
        .ok_or(OperatorError::Preprocess {
            module_id,
            reason: "DRC01 requires frozen local histogram statistics",
        })?;
    generate_local_tone_lut(
        statistics,
        global,
        LocalToneConfig {
            local_strength: 0.75,
            spatial_smoothing_passes: 1,
            minimum_samples: 16,
        },
    )
    .map(Some)
    .map_err(|_| OperatorError::Preprocess {
        module_id,
        reason: "failed to generate DRC local tone LUT",
    })
}

fn write_f32(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

pub(crate) fn encode_f32(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}
