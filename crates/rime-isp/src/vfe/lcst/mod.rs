mod lcst00;
mod lcst00_postprocess;
mod lcst00_preprocess;
mod lcst_common;

use crate::{LcstProducerDefinition, StaticLcstProducer};

pub use lcst00::METHOD_00;

pub const DEFINITION: LcstProducerDefinition = LcstProducerDefinition {
    id: "lcst",
    label: "LCST",
    default_method: "00",
    methods: &[METHOD_00],
};

pub static PRODUCER: StaticLcstProducer = StaticLcstProducer {
    definition: &DEFINITION,
};

pub use lcst_common::{
    D50_GAINS, average_rggb_reference, block_center, cfa_channel, filtered_luma_at, histogram_bin,
    histogram_reference, partition_bounds, validate_source,
};
