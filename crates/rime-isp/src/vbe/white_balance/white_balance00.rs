use crate::operator::{MethodManifest, OperatorPort, method_manifest, shader};
use crate::operator::{
    ModuleParameterPacket, OperatorError, PostprocessContext, PreprocessContext,
};
use rime_core::{ResourceFormat, SignalDomain};

fn preprocess_entry(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    super::wbc00_preprocess::preprocess(context, module_id, method)
}
fn postprocess_entry(context: &mut PostprocessContext) -> Result<(), OperatorError> {
    super::wbc00_postprocess::postprocess(context)
}

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "wbc_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "red_gain green_gain blue_gain",
    shader(
        "00",
        include_str!("wbc00.wgsl"),
        "wbc_main",
        crate::operator::ShaderBindings {
            input: 0,
            output: 1,
            uniform: Some(2),
        },
    ),
    preprocess_entry,
    postprocess_entry,
);
