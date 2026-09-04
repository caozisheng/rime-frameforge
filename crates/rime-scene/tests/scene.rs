use rime_scene::{
    SceneBrightnessSource, SceneInput, SceneLabel, SceneLabelSet, SceneProfile, classify_scene,
    derive_scene_meta, ev100_capture,
};

#[test]
fn calculates_apex_ev_from_aperture_and_exposure_time() {
    let input = SceneInput {
        aperture_f_number: Some(2.0),
        exposure_time_seconds: Some(0.25),
        ..SceneInput::default()
    };

    let ev = ev100_capture(&input).expect("valid exposure settings");

    assert!((ev - 4.0).abs() < 1e-12);
}

#[test]
fn rejects_non_positive_exposure_inputs() {
    let aperture_error = ev100_capture(&SceneInput {
        aperture_f_number: Some(0.0),
        exposure_time_seconds: Some(1.0),
        ..SceneInput::default()
    })
    .expect_err("zero aperture must be rejected");
    assert_eq!(
        aperture_error.to_string(),
        "aperture f-number must be finite and positive"
    );

    let time_error = ev100_capture(&SceneInput {
        aperture_f_number: Some(2.0),
        exposure_time_seconds: Some(-1.0),
        ..SceneInput::default()
    })
    .expect_err("negative exposure time must be rejected");
    assert_eq!(
        time_error.to_string(),
        "exposure time must be finite and positive"
    );
}

#[test]
fn keeps_scene_brightness_exposure_bias_and_iso_as_separate_values() {
    let meta = derive_scene_meta(
        &SceneInput {
            frame_index: 7,
            aperture_f_number: Some(2.0),
            exposure_time_seconds: Some(0.25),
            iso: Some(400.0),
            brightness_value: Some(6.0),
            exposure_bias_ev: Some(1.0),
            analog_gain: Some(4.0),
            digital_gain: Some(1.5),
            ..SceneInput::default()
        },
        &SceneProfile::default(),
    )
    .expect("valid scene input");

    assert_eq!(meta.frame_index, 7);
    assert_eq!(meta.scene_brightness.ev_apex, Some(6.0));
    assert_eq!(
        meta.scene_brightness.source,
        SceneBrightnessSource::Measured
    );
    assert_eq!(meta.exposure_deviation_ev, Some(1.0));
    assert_eq!(meta.iso, Some(400.0));
    assert_eq!(meta.analog_gain, Some(4.0));
    assert_eq!(meta.digital_gain, Some(1.5));
}

#[test]
fn classifies_scene_by_configured_brightness_ranges() {
    let meta = derive_scene_meta(
        &SceneInput {
            brightness_value: Some(9.0),
            ..SceneInput::default()
        },
        &SceneProfile::default(),
    )
    .expect("valid scene input");
    let labels = SceneLabelSet::new(vec![
        SceneLabel::new("night", -8.0, 4.0).expect("valid label"),
        SceneLabel::new("daylight", 4.0, 16.0).expect("valid label"),
    ])
    .expect("non-overlapping labels");

    assert_eq!(
        classify_scene(&meta, &labels).map(|label| label.id()),
        Some("daylight")
    );
}
