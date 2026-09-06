#![expect(
    clippy::cast_precision_loss,
    reason = "the small deterministic test ramp is exactly bounded"
)]

use rime_isp::primitives::{
    guided_filter::{GUIDED_FILTER_WGSL, guided_filter},
    pyramid::PyramidImage,
};

#[test]
fn guided_filter_preserves_constant_fields() {
    let image = PyramidImage::new(9, 7, 1, vec![0.35; 63]).expect("image");
    let output = guided_filter(&image, &image, 3, 0.01).expect("guided filter");

    assert!(
        output
            .data()
            .iter()
            .all(|value| (*value - 0.35).abs() < 1e-5)
    );
}

#[test]
fn guided_filter_radius_zero_is_identity() {
    let values = (0..20).map(|index| index as f32 / 19.0).collect();
    let image = PyramidImage::new(5, 4, 1, values).expect("image");
    let output = guided_filter(&image, &image, 0, 0.01).expect("guided filter");

    assert_eq!(output, image);
}

#[test]
fn self_guided_filter_preserves_a_strong_step_edge() {
    let mut values = Vec::new();
    for _ in 0..6 {
        values.extend([0.0, 0.02, 0.0, 0.02, 0.98, 1.0, 0.98, 1.0]);
    }
    let image = PyramidImage::new(8, 6, 1, values).expect("step image");
    let output = guided_filter(&image, &image, 2, 1e-4).expect("guided filter");

    assert!(output.data()[3] < 0.1);
    assert!(output.data()[4] > 0.9);
}

#[test]
fn guided_filter_shader_declares_separable_statistics_and_apply_passes() {
    for entry in [
        "guided_box_horizontal_main",
        "guided_box_vertical_main",
        "guided_coefficients_main",
        "guided_apply_main",
    ] {
        assert!(GUIDED_FILTER_WGSL.contains(entry), "missing {entry}");
    }
}
