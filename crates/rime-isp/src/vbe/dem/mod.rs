mod dem_00;
mod dem_01;
mod dem_02;
mod dem_03;
mod dem_04;
mod postprocess;
mod preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use dem_00::METHOD_00;
pub use dem_01::METHOD_01;
pub use dem_02::METHOD_02;
pub use dem_03::METHOD_03;
pub use dem_04::METHOD_04;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "dem",
    label: "DEM",
    mode: NodeExecutionMode::Enabled,
    input: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    output_rime_q_profile: Some("s0.12"),
    default_method: "00",
    methods: &[METHOD_00, METHOD_01, METHOD_02, METHOD_03, METHOD_04],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
    shaders: &[
        crate::operator::shader(
            "00",
            include_str!("demosaic_00.wgsl"),
            "demosaic_bilinear_main",
            crate::operator::ShaderBindings {
                input: 1,
                output: 2,
                uniform: Some(0),
            },
        ),
        crate::operator::shader(
            "01",
            include_str!("demosaic_01.wgsl"),
            "demosaic_mhc_main",
            crate::operator::ShaderBindings {
                input: 1,
                output: 2,
                uniform: Some(0),
            },
        ),
        crate::operator::shader(
            "02",
            include_str!("demosaic_02.wgsl"),
            "demosaic_ppg_main",
            crate::operator::ShaderBindings {
                input: 1,
                output: 2,
                uniform: Some(0),
            },
        ),
        crate::operator::shader(
            "03",
            include_str!("demosaic_03.wgsl"),
            "demosaic_vng_main",
            crate::operator::ShaderBindings {
                input: 1,
                output: 2,
                uniform: Some(0),
            },
        ),
        crate::operator::shader(
            "04",
            include_str!("demosaic_04.wgsl"),
            "demosaic_ahd_main",
            crate::operator::ShaderBindings {
                input: 1,
                output: 2,
                uniform: Some(0),
            },
        ),
    ],
    preprocess: preprocess::preprocess,
    postprocess: postprocess::run,
};
