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
#[test]
fn gradient_guided_filter_preserves_constant_fields() {
    let image = PyramidImage::new(11, 9, 1, vec![0.42; 99]).expect("image");
    let output = rime_isp::primitives::guided_filter::gradient_guided_filter(&image, 3, 1.0)
        .expect("gradient guided filter");

    assert!(
        output
            .data()
            .iter()
            .all(|value| (*value - 0.42).abs() < 1e-4)
    );
}

#[test]
fn gradient_guided_filter_preserves_a_strong_step_edge() {
    let mut values = Vec::new();
    for _ in 0..9 {
        values.extend([0.0, 0.01, 0.0, 0.01, 0.99, 1.0, 0.99, 1.0]);
    }
    let image = PyramidImage::new(8, 9, 1, values).expect("step image");
    let output = rime_isp::primitives::guided_filter::gradient_guided_filter(&image, 3, 1.0)
        .expect("gradient guided filter");

    // The edge flanks are intentionally softened toward the box mean by the
    // reference calibration; the contrast across the step itself must hold.
    assert!(
        output.data()[5] - output.data()[2] > 0.55,
        "gradient modulation must preserve step contrast"
    );
}

#[test]
fn gradient_guided_filter_rejects_multichannel_input() {
    let image = PyramidImage::new(4, 4, 3, vec![0.5; 48]).expect("rgb image");
    let error = rime_isp::primitives::guided_filter::gradient_guided_filter(&image, 3, 1.0)
        .expect_err("multichannel must be rejected");

    assert_eq!(
        error,
        rime_isp::primitives::guided_filter::GuidedFilterError::GradientRequiresMonoSelfGuided
    );
}

#[test]
fn shared_statistics_segment_declares_the_gf_component_contract() {
    let shared = rime_isp::primitives::guided_filter::GUIDED_FILTER_SHARED_FUNCTIONS;
    for entry in [
        "gf_local_variance",
        "gf_gradient_chi",
        "gf_dynamic_epsilon",
        "gf_gradient_weight",
        "gf_gradient_gamma",
        "gf_box_moments",
    ] {
        assert!(shared.contains(entry), "missing {entry}");
    }
}
