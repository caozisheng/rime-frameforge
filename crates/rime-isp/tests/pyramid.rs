#![expect(
    clippy::cast_precision_loss,
    reason = "the small deterministic test image extents are exactly bounded"
)]

use rime_isp::primitives::pyramid::{
    PyramidImage, level_extents, pyr_dec_gaussian, pyr_dec_laplacian, pyr_rec,
};

fn image(width: u32, height: u32) -> PyramidImage {
    let data = (0..width * height)
        .map(|index| index as f32 / (width * height - 1).max(1) as f32)
        .collect();
    PyramidImage::new(width, height, 1, data).expect("valid image")
}

#[test]
fn toolbox_level_extents_use_floor_half_with_a_one_pixel_minimum() {
    assert_eq!(level_extents(7, 5, 4), vec![(7, 5), (3, 2), (1, 1), (1, 1)]);
}

#[test]
fn gaussian_pyramid_preserves_constant_images_and_expected_extents() {
    let source = PyramidImage::new(7, 5, 1, vec![0.25; 35]).expect("constant image");
    let levels = pyr_dec_gaussian(&source, 3).expect("Gaussian pyramid");

    assert_eq!(
        levels.iter().map(PyramidImage::extent).collect::<Vec<_>>(),
        vec![(7, 5), (3, 2), (1, 1)]
    );
    assert!(
        levels
            .iter()
            .flat_map(PyramidImage::data)
            .all(|value| (*value - 0.25).abs() < 1e-5)
    );
}

#[test]
fn laplacian_decomposition_reconstructs_odd_sized_input() {
    let source = image(7, 5);
    let pyramid = pyr_dec_laplacian(&source, 3).expect("Laplacian pyramid");
    let reconstructed = pyr_rec(&pyramid).expect("reconstruction");

    assert_eq!(reconstructed.extent(), source.extent());
    let max_error = reconstructed
        .data()
        .iter()
        .zip(source.data())
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_error < 1e-5, "max reconstruction error {max_error}");
}

#[test]
fn pyramid_rejects_zero_levels() {
    let error = pyr_dec_gaussian(&image(2, 2), 0).expect_err("zero levels are invalid");
    assert_eq!(error.to_string(), "pyramid level count must be positive");
}

#[test]
fn gaussian_pyramid_matches_matlab_imresize_reference_vector() {
    let levels = pyr_dec_gaussian(&image(7, 5), 3).expect("Gaussian pyramid");
    let expected_level_1 = [
        0.158_283_86,
        0.228_894_13,
        0.299_504_4,
        0.700_495_6,
        0.771_105_9,
        0.841_716_2,
    ];
    for (actual, expected) in levels[1].data().iter().zip(expected_level_1) {
        assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
    }
    assert!((levels[2].data()[0] - 0.5).abs() < 1e-5);
}
