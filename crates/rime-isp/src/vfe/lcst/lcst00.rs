use super::{lcst00_postprocess, lcst00_preprocess};
use crate::{
    LcstMethodManifest, ShaderAsset,
    operator::{
        ShaderBindingAccess, ShaderBindingKind, ShaderBindings, ShaderStageAsset,
        ShaderStageBinding,
    },
};

const AVERAGE_BINDINGS: &[ShaderStageBinding] = &[
    ShaderStageBinding {
        binding: 0,
        resource: "input",
        kind: ShaderBindingKind::Texture,
        access: ShaderBindingAccess::Read,
    },
    ShaderStageBinding {
        binding: 1,
        resource: "parameters",
        kind: ShaderBindingKind::UniformBuffer,
        access: ShaderBindingAccess::Read,
    },
    ShaderStageBinding {
        binding: 2,
        resource: "average_rggb",
        kind: ShaderBindingKind::StorageBuffer,
        access: ShaderBindingAccess::Write,
    },
];

const HISTOGRAM_BINDINGS: &[ShaderStageBinding] = &[
    ShaderStageBinding {
        binding: 0,
        resource: "input",
        kind: ShaderBindingKind::Texture,
        access: ShaderBindingAccess::Read,
    },
    ShaderStageBinding {
        binding: 1,
        resource: "parameters",
        kind: ShaderBindingKind::UniformBuffer,
        access: ShaderBindingAccess::Read,
    },
    ShaderStageBinding {
        binding: 3,
        resource: "luma_histogram",
        kind: ShaderBindingKind::StorageBuffer,
        access: ShaderBindingAccess::Write,
    },
];

const STAGES: &[ShaderStageAsset] = &[
    ShaderStageAsset {
        entry_point: "lcst_average_main",
        bindings: AVERAGE_BINDINGS,
    },
    ShaderStageAsset {
        entry_point: "lcst_histogram_main",
        bindings: HISTOGRAM_BINDINGS,
    },
];

pub const METHOD_00: LcstMethodManifest = LcstMethodManifest {
    method: "00",
    shader: ShaderAsset {
        method: "00",
        source: include_str!("lcst00.wgsl"),
        entry_point: "lcst_average_main",
        bindings: ShaderBindings {
            input: 0,
            output: 2,
            uniform: Some(1),
        },
        workgroup_size: [1, 1, 1],
        stages: STAGES,
    },
    preprocess: lcst00_preprocess::run,
    decode: lcst00_postprocess::decode,
    average_dispatch: [64, 48, 1],
    histogram_dispatch: [16, 16, 1],
};
