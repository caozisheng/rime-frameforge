use crate::operator::{
    ModuleParameterPacket, ModuleParameterResource, OperatorError, PreprocessContext,
};

const RECORD_FLOATS: usize = 7;
const RECORD_BYTES: usize = RECORD_FLOATS * size_of::<f32>();

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let opcode_count =
        u32::try_from(context.vignette_radial.len()).map_err(|_| OperatorError::Preprocess {
            module_id,
            reason: "too many FixVignetteRadial opcodes",
        })?;
    if context.vignette_radial.iter().any(|opcode| {
        opcode
            .coefficients
            .iter()
            .any(|value| !value.is_finite() || value.abs() > f64::from(f32::MAX))
            || opcode
                .optical_center
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    }) {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "FixVignetteRadial values must be finite, GPU-representable, and centers within [0, 1]",
        });
    }
    let mut uniform = [0_u8; 16];
    uniform[0..4].copy_from_slice(&opcode_count.to_ne_bytes());
    uniform[4..8].copy_from_slice(&context.width.to_ne_bytes());
    uniform[8..12].copy_from_slice(&context.height.to_ne_bytes());
    let mut packet = ModuleParameterPacket::new(module_id, method, context.identity, &uniform)?;

    let record_count = context.vignette_radial.len().max(1);
    let mut records = Vec::with_capacity(record_count * RECORD_BYTES);
    for opcode in &context.vignette_radial {
        for value in opcode.coefficients.into_iter().chain(opcode.optical_center) {
            records.extend_from_slice(&gpu_value(value).to_ne_bytes());
        }
    }
    records.resize(record_count * RECORD_BYTES, 0);
    packet.push_resource(ModuleParameterResource::new(
        "vignette_radial",
        [
            u32::try_from(record_count).expect("record count fits u32"),
            1,
            1,
        ],
        records,
    ))?;
    Ok(packet)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "values are validated as finite and within the f32 range before packet encoding"
)]
fn gpu_value(value: f64) -> f32 {
    value as f32
}
