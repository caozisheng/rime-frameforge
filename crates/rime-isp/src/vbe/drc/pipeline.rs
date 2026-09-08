use crate::operator::{
    ShaderBindingAccess, ShaderBindingKind, ShaderStageAsset, ShaderStageBinding,
};

const fn read(binding: u32, resource: &'static str, kind: ShaderBindingKind) -> ShaderStageBinding {
    ShaderStageBinding {
        binding,
        resource,
        kind,
        access: ShaderBindingAccess::Read,
    }
}

const fn write(
    binding: u32,
    resource: &'static str,
    kind: ShaderBindingKind,
) -> ShaderStageBinding {
    ShaderStageBinding {
        binding,
        resource,
        kind,
        access: ShaderBindingAccess::Write,
    }
}

const PREFILTER: &[ShaderStageBinding] = &[
    read(0, "scalars", ShaderBindingKind::UniformBuffer),
    read(1, "input_raw", ShaderBindingKind::Texture),
    write(4, "analysis_level_0", ShaderBindingKind::StorageTexture),
];
const DOWNSAMPLE: &[ShaderStageBinding] = &[
    read(0, "scalars", ShaderBindingKind::UniformBuffer),
    read(1, "finer_level", ShaderBindingKind::Texture),
    write(4, "coarser_level", ShaderBindingKind::StorageTexture),
];
const RECONSTRUCT: &[ShaderStageBinding] = &[
    read(0, "scalars", ShaderBindingKind::UniformBuffer),
    read(1, "finer_level", ShaderBindingKind::Texture),
    read(2, "coarser_level", ShaderBindingKind::Texture),
    read(3, "coarser_base", ShaderBindingKind::Texture),
    write(4, "candidate_base", ShaderBindingKind::StorageTexture),
];
const GUIDED_COEFFICIENTS: &[ShaderStageBinding] = &[
    read(0, "scalars", ShaderBindingKind::UniformBuffer),
    read(1, "candidate_base", ShaderBindingKind::Texture),
    write(5, "guided_coefficients", ShaderBindingKind::StorageTexture),
];
const GUIDED_APPLY: &[ShaderStageBinding] = &[
    read(0, "scalars", ShaderBindingKind::UniformBuffer),
    read(1, "guided_coefficients", ShaderBindingKind::Texture),
    read(2, "candidate_base", ShaderBindingKind::Texture),
    write(4, "filtered_base", ShaderBindingKind::StorageTexture),
    read(8, "modulation_luts", ShaderBindingKind::StorageBuffer),
];
const COMBINE_GLOBAL: &[ShaderStageBinding] = &[
    read(0, "scalars", ShaderBindingKind::UniformBuffer),
    read(1, "input_raw", ShaderBindingKind::Texture),
    read(2, "analysis_level_0", ShaderBindingKind::Texture),
    read(3, "filtered_base", ShaderBindingKind::Texture),
    write(4, "output_raw", ShaderBindingKind::StorageTexture),
    read(6, "tone_lut_global", ShaderBindingKind::StorageBuffer),
    read(8, "modulation_luts", ShaderBindingKind::StorageBuffer),
];
const COMBINE_LOCAL: &[ShaderStageBinding] = &[
    read(0, "scalars", ShaderBindingKind::UniformBuffer),
    read(1, "input_raw", ShaderBindingKind::Texture),
    read(2, "analysis_level_0", ShaderBindingKind::Texture),
    read(3, "filtered_base", ShaderBindingKind::Texture),
    write(4, "output_raw", ShaderBindingKind::StorageTexture),
    read(6, "tone_lut_global", ShaderBindingKind::StorageBuffer),
    read(7, "tone_lut_local", ShaderBindingKind::StorageBuffer),
    read(8, "modulation_luts", ShaderBindingKind::StorageBuffer),
];

const COMMON_STAGES: [ShaderStageAsset; 5] = [
    ShaderStageAsset {
        entry_point: "drc_prefilter_main",
        bindings: PREFILTER,
    },
    ShaderStageAsset {
        entry_point: "pyramid_downsample_main",
        bindings: DOWNSAMPLE,
    },
    ShaderStageAsset {
        entry_point: "pyramid_reconstruct_main",
        bindings: RECONSTRUCT,
    },
    ShaderStageAsset {
        entry_point: "guided_coefficients_main",
        bindings: GUIDED_COEFFICIENTS,
    },
    ShaderStageAsset {
        entry_point: "guided_apply_vertical_main",
        bindings: GUIDED_APPLY,
    },
];

pub const GLOBAL_STAGES: &[ShaderStageAsset] = &[
    COMMON_STAGES[0],
    COMMON_STAGES[1],
    COMMON_STAGES[2],
    COMMON_STAGES[3],
    COMMON_STAGES[4],
    ShaderStageAsset {
        entry_point: "drc_combine_global_main",
        bindings: COMBINE_GLOBAL,
    },
];

pub const LOCAL_STAGES: &[ShaderStageAsset] = &[
    COMMON_STAGES[0],
    COMMON_STAGES[1],
    COMMON_STAGES[2],
    COMMON_STAGES[3],
    COMMON_STAGES[4],
    ShaderStageAsset {
        entry_point: "drc_combine_local_main",
        bindings: COMBINE_LOCAL,
    },
];
