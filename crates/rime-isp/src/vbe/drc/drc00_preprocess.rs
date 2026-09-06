#![expect(
    clippy::cast_possible_truncation,
    reason = "validated exposure gains and fixed LUT extents are narrowed to the GPU f32/u32 contract"
)]

use crate::operator::{
    ModuleParameterPacket, ModuleParameterResource, OperatorError, PreprocessContext,
};

use super::{
    DrcExposureInputs, LocalToneConfig, generate_global_tone_lut, generate_local_tone_lut,
    resolve_drc_exposure,
};
use crate::vbe::white_balance::{WhiteBalanceError, WhiteBalanceMetadata, white_balance_gains};

const TONE_SAMPLES: usize = 257;
const DEFAULT_KNEE: f32 = 1.0;
const DEFAULT_AMPLIFIER: f32 = 1.0;
const DEFAULT_MIN_RATIO: f32 = 1.0 / 256.0;
const DEFAULT_LUMA_GUARD: f32 = 1.0 / 65_536.0;
const DEFAULT_LEVEL_COUNT: u32 = 3;

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
    let rgb_gains = resolve_cfa_gains(context, module_id)?;
    let global = generate_global_tone_lut(drc_gain, DEFAULT_KNEE, TONE_SAMPLES).map_err(|_| {
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
    let mut uniform = [0_u8; 48];
    write_f32(&mut uniform, 0, drc_gain);
    write_f32(&mut uniform, 4, DEFAULT_KNEE);
    write_f32(&mut uniform, 8, DEFAULT_AMPLIFIER);
    write_f32(&mut uniform, 12, DEFAULT_LUMA_GUARD);
    write_f32(&mut uniform, 16, DEFAULT_MIN_RATIO);
    write_f32(&mut uniform, 20, drc_gain * 4.0);
    write_u32(&mut uniform, 24, DEFAULT_LEVEL_COUNT);
    let feature_flags = 1 | ((tiles_x & 0xff) << 8) | ((tiles_y & 0xff) << 16);
    write_u32(&mut uniform, 28, feature_flags);
    for (index, channel) in context.cfa_pattern.into_iter().enumerate() {
        let gain = rgb_gains
            .get(channel as usize)
            .copied()
            .ok_or(OperatorError::Preprocess {
                module_id,
                reason: "invalid CFA channel for DRC guide",
            })?;
        write_f32(&mut uniform, 32 + index * 4, gain);
    }
    let mut packet = ModuleParameterPacket::new(module_id, method, context.identity, &uniform)?;
    packet.push_resource(ModuleParameterResource::new(
        "tone_lut_global",
        [TONE_SAMPLES as u32, 1, 1],
        encode_f32(global.values()),
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

fn resolve_gain(
    context: &PreprocessContext,
    module_id: &'static str,
) -> Result<f32, OperatorError> {
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
    let drc_gain = exposure.drc_gain as f32;
    if !drc_gain.is_finite() || drc_gain <= 0.0 {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "DRC gain is outside the GPU f32 domain",
        });
    }
    Ok(drc_gain)
}

fn resolve_cfa_gains(
    context: &PreprocessContext,
    module_id: &'static str,
) -> Result<[f32; 3], OperatorError> {
    let metadata = WhiteBalanceMetadata {
        as_shot_neutral: context.as_shot_neutral,
        as_shot_white_xy: context.as_shot_white_xy,
        color_matrix1: context.color_matrix1,
        color_matrix2: context.color_matrix2,
        analog_balance: context.analog_balance,
    };
    match white_balance_gains(&metadata) {
        Ok(gains) => Ok([gains.red, gains.green, gains.blue]),
        Err(WhiteBalanceError::MissingSource) => Ok([1.0; 3]),
        Err(_) => Err(OperatorError::Preprocess {
            module_id,
            reason: "invalid white-balance metadata for DRC guide",
        }),
    }
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
