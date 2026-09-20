#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "fixed tintless mesh dimensions and bounded audit counters fit GPU packet fields"
)]
use crate::operator::{
    ModuleParameterPacket, ModuleParameterResource, OperatorError, PreprocessContext,
};

use super::{
    TINTLESS_MESH_HEIGHT, TINTLESS_MESH_WIDTH, TintlessError, TintlessGainMesh, estimate_gain_mesh,
};

const UNIFORM_BYTES: usize = 48;
const GAIN_MIN: f32 = 0.5;
const GAIN_MAX: f32 = 2.0;

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let (mesh, cold_start) = match context.lcst_statistics.as_ref() {
        Some(statistics) => (
            estimate_gain_mesh(
                statistics,
                context.identity,
                [context.width, context.height],
                context.cfa_pattern,
            )
            .map_err(|error| OperatorError::Preprocess {
                module_id,
                reason: error_reason(error),
            })?,
            false,
        ),
        None => (
            TintlessGainMesh::identity(
                context.identity,
                context.identity,
                [context.width, context.height],
                context.cfa_pattern,
            ),
            true,
        ),
    };

    let mut uniform = [0_u8; UNIFORM_BYTES];
    write_u32(&mut uniform, 0, context.width);
    write_u32(&mut uniform, 4, context.height);
    write_u32(&mut uniform, 8, TINTLESS_MESH_WIDTH as u32);
    write_u32(&mut uniform, 12, TINTLESS_MESH_HEIGHT as u32);
    for (index, value) in context.cfa_pattern.into_iter().enumerate() {
        write_u32(&mut uniform, 16 + index * 4, value);
    }
    write_f32(&mut uniform, 32, GAIN_MIN);
    write_f32(&mut uniform, 36, GAIN_MAX);
    write_u32(&mut uniform, 40, u32::from(cold_start));

    let mut packet = ModuleParameterPacket::new(module_id, method, context.identity, &uniform)?;
    packet.push_resource(ModuleParameterResource::new(
        "gain_mesh",
        [TINTLESS_MESH_WIDTH as u32, TINTLESS_MESH_HEIGHT as u32, 2],
        encode_f32(mesh.entries()),
    ))?;
    packet.push_resource(ModuleParameterResource::host_only(
        "audit",
        [5, 1, 1],
        encode_audit(&mesh),
    ))?;
    Ok(packet)
}

fn encode_audit(mesh: &TintlessGainMesh) -> Vec<u8> {
    [
        mesh.valid_cells() as f32,
        mesh.qualified_components() as f32,
        mesh.residual_rms()[0],
        mesh.residual_rms()[1],
        mesh.clamp_count() as f32,
    ]
    .into_iter()
    .flat_map(f32::to_ne_bytes)
    .collect()
}

const fn error_reason(error: TintlessError) -> &'static str {
    match error {
        TintlessError::IdentityMismatch => "tintless LCST frame identity is incompatible",
        TintlessError::ExtentMismatch => "tintless LCST source extent is incompatible",
        TintlessError::CfaMismatch => "tintless LCST CFA pattern is incompatible",
        TintlessError::NonFiniteAverage => "tintless LCST average is non-finite",
        TintlessError::InsufficientComponents => {
            "tintless has insufficient qualified hue components"
        }
        TintlessError::SingularSolve => "tintless radial solve is singular",
        TintlessError::NonFiniteSolve => "tintless radial solve is non-finite",
    }
}

fn encode_f32(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn write_f32(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}
