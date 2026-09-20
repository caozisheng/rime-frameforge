#![expect(
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    reason = "fixed LCST fixture dimensions and exact representable values"
)]

use rime_isp::{
    FrameIdentity, LCST_AVERAGE_VALUES, LCST_HISTOGRAM_VALUES, LcstStatisticsError,
    LcstStatisticsPacket,
};

const IDENTITY: FrameIdentity = FrameIdentity {
    frame_index: 7,
    run_revision: 2,
    method_revision: 3,
};

fn valid_packet(width: u32, height: u32) -> LcstStatisticsPacket {
    let averages = vec![0.5; LCST_AVERAGE_VALUES];
    let mut histograms = vec![0_u32; LCST_HISTOGRAM_VALUES];
    for tile_y in 0..16_u32 {
        let y0 = tile_y * height / 16;
        let y1 = (tile_y + 1) * height / 16;
        for tile_x in 0..16_u32 {
            let x0 = tile_x * width / 16;
            let x1 = (tile_x + 1) * width / 16;
            let tile = (tile_y * 16 + tile_x) as usize;
            histograms[tile * 16] = (x1 - x0) * (y1 - y0);
        }
    }
    LcstStatisticsPacket::new(IDENTITY, width, height, [0, 1, 1, 2], averages, histograms)
        .expect("valid LCST packet")
}

#[test]
fn packet_preserves_identity_extent_cfa_and_fixed_layout() {
    let packet = valid_packet(128, 96);
    assert_eq!(packet.identity(), IDENTITY);
    assert_eq!(packet.source_extent(), [128, 96]);
    assert_eq!(packet.cfa_pattern(), [0, 1, 1, 2]);
    assert_eq!(packet.average_rggb().len(), LCST_AVERAGE_VALUES);
    assert_eq!(packet.luma_histograms().len(), LCST_HISTOGRAM_VALUES);
}

#[test]
fn packet_rejects_invalid_lengths_cfa_values_and_histogram_totals() {
    let error = LcstStatisticsPacket::new(
        IDENTITY,
        128,
        96,
        [0, 1, 1, 2],
        vec![0.5; LCST_AVERAGE_VALUES - 1],
        vec![0; LCST_HISTOGRAM_VALUES],
    )
    .expect_err("average length");
    assert_eq!(error, LcstStatisticsError::InvalidAverageLength);

    let error = LcstStatisticsPacket::new(
        IDENTITY,
        128,
        96,
        [0, 1, 2, 2],
        vec![0.5; LCST_AVERAGE_VALUES],
        vec![0; LCST_HISTOGRAM_VALUES],
    )
    .expect_err("CFA permutation");
    assert_eq!(error, LcstStatisticsError::InvalidCfaPattern);

    let error = LcstStatisticsPacket::new(
        IDENTITY,
        128,
        96,
        [0, 1, 1, 2],
        vec![0.5; LCST_AVERAGE_VALUES],
        vec![0; LCST_HISTOGRAM_VALUES],
    )
    .expect_err("histogram totals");
    assert_eq!(error, LcstStatisticsError::InvalidHistogramTotal);
}

#[test]
fn packet_rejects_non_finite_average_and_zero_extent() {
    let mut averages = vec![0.5; LCST_AVERAGE_VALUES];
    averages[9] = f32::NAN;
    let error = LcstStatisticsPacket::new(
        IDENTITY,
        128,
        96,
        [0, 1, 1, 2],
        averages,
        vec![0; LCST_HISTOGRAM_VALUES],
    )
    .expect_err("finite average");
    assert_eq!(error, LcstStatisticsError::NonFiniteAverage);

    let error = LcstStatisticsPacket::new(
        IDENTITY,
        0,
        96,
        [0, 1, 1, 2],
        vec![0.5; LCST_AVERAGE_VALUES],
        vec![0; LCST_HISTOGRAM_VALUES],
    )
    .expect_err("extent");
    assert_eq!(error, LcstStatisticsError::InvalidSourceExtent);
}

#[test]
fn cpu_reference_resolves_all_cfa_phases_and_constant_channel_averages() {
    for cfa in [[0, 1, 1, 2], [1, 0, 2, 1], [1, 2, 0, 1], [2, 1, 1, 0]] {
        let width = 128;
        let height = 96;
        let mut samples = vec![0.0; width * height];
        for y in 0..height {
            for x in 0..width {
                let channel =
                    rime_isp::vfe::lcst::cfa_channel(cfa, x as u32, y as u32).expect("valid CFA");
                samples[y * width + x] = [0.1, 0.2, 0.3, 0.4][channel];
            }
        }
        let averages =
            rime_isp::vfe::lcst::average_rggb_reference(&samples, width as u32, height as u32, cfa)
                .expect("averages");
        for block in averages.chunks_exact(4) {
            assert_eq!(block, [0.1, 0.2, 0.3, 0.4]);
        }
    }
}

#[test]
fn cpu_reference_uses_width_first_partitions_and_rejects_missing_cfa_sites() {
    assert_eq!(rime_isp::vfe::lcst::partition_bounds(1, 64, 130), [2, 4]);
    assert_eq!(rime_isp::vfe::lcst::partition_bounds(1, 48, 98), [2, 4]);
    assert_eq!(rime_isp::vfe::lcst::block_center(1, 64, 130), 2.5);
    assert_eq!(
        rime_isp::vfe::lcst::validate_source(64, 48, [0, 1, 1, 2]),
        Err(LcstStatisticsError::MissingCfaChannel)
    );
}

#[test]
fn cpu_reference_gaussian_clamps_edges_and_histogram_bins_are_stable() {
    let mut samples = vec![0.0; 128 * 96];
    samples[0] = 1.0;
    let corner = rime_isp::vfe::lcst::filtered_luma_at(&samples, 128, 96, [0, 1, 1, 2], 0, 0)
        .expect("corner luma");
    assert_eq!(corner, 2.804_687_5 * 9.0 / 16.0);
    assert_eq!(rime_isp::vfe::lcst::histogram_bin(-1.0), 0);
    assert_eq!(rime_isp::vfe::lcst::histogram_bin(1.0 / 16.0), 1);
    assert_eq!(rime_isp::vfe::lcst::histogram_bin(15.0 / 16.0), 15);
    assert_eq!(rime_isp::vfe::lcst::histogram_bin(1.0), 15);
    assert_eq!(rime_isp::vfe::lcst::histogram_bin(2.0), 15);

    let histograms =
        rime_isp::vfe::lcst::histogram_reference(&vec![0.0; 130 * 98], 130, 98, [0, 1, 1, 2])
            .expect("non-divisible histogram");
    assert_eq!(histograms.iter().sum::<u32>(), 130 * 98);
}
#[test]
fn lcst_registry_resolves_method_zero_and_fixed_dispatches() {
    let producer = rime_isp::lcst_producer_by_id("lcst").expect("LCST producer");
    assert_eq!(producer.definition().id, "lcst");
    let method = producer.method("00").expect("LCST method 00");
    assert_eq!(method.average_dispatch, [64, 48, 1]);
    assert_eq!(method.histogram_dispatch, [16, 16, 1]);
    assert_eq!(method.shader.entry_point, "lcst_average_main");
}
