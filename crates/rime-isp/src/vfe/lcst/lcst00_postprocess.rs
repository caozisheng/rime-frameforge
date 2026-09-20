use crate::{
    FrameIdentity, LCST_AVERAGE_BYTES, LCST_AVERAGE_VALUES, LCST_HISTOGRAM_VALUES,
    LCST_PAYLOAD_BYTES, LcstStatisticsError, LcstStatisticsPacket,
};

pub(crate) fn decode(
    identity: FrameIdentity,
    source_extent: [u32; 2],
    cfa_pattern: [u32; 4],
    bytes: &[u8],
) -> Result<LcstStatisticsPacket, LcstStatisticsError> {
    if bytes.len() != LCST_PAYLOAD_BYTES {
        return Err(LcstStatisticsError::InvalidPayloadLength);
    }
    let averages = bytes[..LCST_AVERAGE_BYTES]
        .chunks_exact(4)
        .map(|word| f32::from_le_bytes(word.try_into().expect("four-byte LCST average")))
        .collect::<Vec<_>>();
    let histograms = bytes[LCST_AVERAGE_BYTES..]
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("four-byte LCST histogram")))
        .collect::<Vec<_>>();
    debug_assert_eq!(averages.len(), LCST_AVERAGE_VALUES);
    debug_assert_eq!(histograms.len(), LCST_HISTOGRAM_VALUES);
    LcstStatisticsPacket::new(
        identity,
        source_extent[0],
        source_extent[1],
        cfa_pattern,
        averages,
        histograms,
    )
}
