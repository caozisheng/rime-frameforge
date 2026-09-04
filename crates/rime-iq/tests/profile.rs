use rime_iq::{ModuleCatalogEntry, ProfileError, TuningProfile};

fn catalog() -> Vec<ModuleCatalogEntry> {
    vec![
        ModuleCatalogEntry::new("vbe.dem", "dem", "04", "dem04-v1"),
        ModuleCatalogEntry::new("vpe.mctf[1]", "mctf", "00", "mctf-v1")
            .with_binding_group("mctf_1"),
    ]
}

const PROFILE: &str = r#"
kind: rime.tuning_profile
schema_version: 1
profile:
  id: test-profile
  name: Test Profile
  profile_revision: 3
pipeline:
  graph_id: normal
  manifest_revision: normal-v1
  base_iq_set: factory-default
camera:
  profile_id: test-camera
  calibration_revision: cal-v1
modules:
  vbe.dem:
    module_id: dem
    method: "04"
    tuning: override
    table:
      schema_version: 1
      parameter_schema_revision: dem04-v1
      axes:
        - id: scene_brightness_ev
          source: scene_meta.scene_brightness.ev_apex
          unit: EV
          knots: [-4.0, 0.0]
      effects:
        ahd_l_threshold:
          unit: lab_delta_l
          values: [1.0, 2.0]
        ahd_c_threshold_sq:
          unit: lab_delta_ab_squared
          values: [3.0, 4.0]
      modulation_curves: []
  vpe.mctf[1]:
    module_id: mctf
    method: "00"
    binding_group: mctf_1
    tuning: inherit
"#;

#[test]
fn parses_and_resolves_full_profile_entries() {
    let profile = TuningProfile::from_yaml(PROFILE).expect("profile parses");
    let resolved = profile.resolve(&catalog()).expect("profile resolves");

    assert_eq!(resolved.profile_id(), "test-profile");
    assert!(resolved.module("vbe.dem").is_some_and(|entry| entry.is_override()));
    assert!(resolved.module("vpe.mctf[1]").is_some_and(|entry| entry.is_inherit()));
}

#[test]
fn rejects_unknown_module_and_bad_table_shape() {
    let unknown = PROFILE.replace("vpe.mctf[1]:", "vpe.unknown:");
    assert!(matches!(
        TuningProfile::from_yaml(&unknown)
            .expect("profile parses")
            .resolve(&catalog()),
        Err(ProfileError::UnknownModule { .. })
    ));

    let bad_shape = PROFILE.replace("values: [1.0, 2.0]", "values: [1.0]");
    assert!(matches!(
        TuningProfile::from_yaml(&bad_shape)
            .expect("profile parses")
            .resolve(&catalog()),
        Err(ProfileError::InvalidTableShape { .. })
    ));
}

#[test]
fn yaml_round_trip_preserves_profile_identity_and_revision() {
    let profile = TuningProfile::from_yaml(PROFILE).expect("profile parses");
    let encoded = profile.to_yaml().expect("profile serializes");
    let restored = TuningProfile::from_yaml(&encoded).expect("round trip parses");

    assert_eq!(restored.profile_id(), "test-profile");
    assert_eq!(restored.profile_revision(), 3);
    assert_eq!(restored.modules().len(), 2);
}
