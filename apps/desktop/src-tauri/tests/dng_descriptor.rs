#![expect(
    dead_code,
    reason = "the command module is included to exercise descriptor serialization"
)]

#[path = "../src/dng_command.rs"]
mod dng_command;

use std::path::Path;

use gamut_dng::{DngRewrite, Opcode, OpcodeList, Value, opcode_id, tags};
use rime_dng::DngReader;

const GH5S: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../pipeline/normal/P1020601.dng"
);

#[test]
fn descriptor_serializes_complete_metadata_and_filename() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S), 0)
        .expect("GH5S frame must decode");
    let descriptor = dng_command::descriptor_from_frame(&frame, Path::new(GH5S))
        .expect("descriptor must preprocess white balance");
    let json = serde_json::to_value(descriptor).expect("descriptor serializes");

    assert_eq!(json["fileName"], "P1020601.dng");
    assert_eq!(
        json["metadata"]["colorMatrix1"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        json["metadata"]["asShotNeutral"].as_array().map(Vec::len),
        Some(3)
    );
    assert!(json["metadata"]["asShotWhiteXY"].is_null());
    assert_eq!(json["whiteBalanceGains"].as_array().map(Vec::len), Some(3));
    assert!(
        json["metadata"]["ifd0Extra"]
            .as_array()
            .is_some_and(|tags| !tags.is_empty())
    );
    assert!(
        json["metadata"]["rawExtra"]
            .as_array()
            .is_some_and(|tags| !tags.is_empty())
    );
}

#[test]
fn descriptor_serializes_all_opcode_lists_without_losing_unknown_parameters() {
    let data = std::fs::read(GH5S).expect("GH5S fixture must exist");
    let mut rewrite = DngRewrite::open(&data).expect("fixture must be rewriteable");
    let raw_ifd = rewrite
        .file_mut()
        .ifds
        .first_mut()
        .expect("IFD0")
        .sub_ifds_mut()
        .first_mut()
        .expect("raw SubIFD group")
        .ifds
        .first_mut()
        .expect("raw IFD");
    let mut list1 = OpcodeList::new();
    list1.push(Opcode {
        id: 65_001,
        spec_version: [1, 4, 0, 0],
        flags: Opcode::FLAG_OPTIONAL | Opcode::FLAG_PREVIEW_SKIP,
        parameters: vec![0xde, 0xad],
    });
    raw_ifd.set(tags::OPCODE_LIST1, Value::Undefined(list1.to_bytes()));

    let mut list2 = OpcodeList::new();
    list2.push(Opcode {
        id: 65_002,
        spec_version: [1, 4, 0, 0],
        flags: Opcode::FLAG_OPTIONAL | Opcode::FLAG_PREVIEW_SKIP,
        parameters: vec![0xbe, 0xef, 0x01],
    });
    let mut vignette_parameters = Vec::new();
    for value in [0.1_f64, 0.02, 0.003, 0.0004, 0.00005, 0.45, 0.55] {
        vignette_parameters.extend_from_slice(&value.to_be_bytes());
    }
    list2.push(Opcode {
        id: opcode_id::FIX_VIGNETTE_RADIAL,
        spec_version: [1, 3, 0, 0],
        flags: 0,
        parameters: vignette_parameters,
    });
    let mut skipped_vignette_parameters = Vec::new();
    for value in [0.2_f64, 0.03, 0.004, 0.0005, 0.00006, 0.4, 0.6] {
        skipped_vignette_parameters.extend_from_slice(&value.to_be_bytes());
    }
    list2.push(Opcode {
        id: opcode_id::FIX_VIGNETTE_RADIAL,
        spec_version: [1, 3, 0, 0],
        flags: Opcode::FLAG_PREVIEW_SKIP,
        parameters: skipped_vignette_parameters,
    });
    let mut gain_map_parameters = Vec::new();
    // area_spec: t, l, b, r, plane, planes, rowPitch, colPitch (DNG SDK).
    for value in [0_i32, 0, 2776, 3744] {
        gain_map_parameters.extend_from_slice(&value.to_be_bytes());
    }
    for value in [0_u32, 1, 1, 1] {
        gain_map_parameters.extend_from_slice(&value.to_be_bytes());
    }
    // Mesh: points 2x3, spacing (0.5, 0.25), origin (0.1, 0.05), planes 2.
    gain_map_parameters.extend_from_slice(&2_u32.to_be_bytes());
    gain_map_parameters.extend_from_slice(&3_u32.to_be_bytes());
    for value in [0.5_f64, 0.25, 0.1, 0.05] {
        gain_map_parameters.extend_from_slice(&value.to_be_bytes());
    }
    gain_map_parameters.extend_from_slice(&2_u32.to_be_bytes());
    for entry in [
        1.0_f32, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.25, 3.5, 3.75,
    ] {
        gain_map_parameters.extend_from_slice(&entry.to_be_bytes());
    }
    list2.push(Opcode {
        id: opcode_id::GAIN_MAP,
        spec_version: [1, 3, 0, 0],
        flags: 0,
        parameters: gain_map_parameters,
    });
    raw_ifd.set(tags::OPCODE_LIST2, Value::Undefined(list2.to_bytes()));
    let mut warp_parameters = 1_u32.to_be_bytes().to_vec();
    for value in [1.0_f64, 0.01, 0.001, 0.0, 0.0, 0.0, 0.49, 0.51] {
        warp_parameters.extend_from_slice(&value.to_be_bytes());
    }
    let mut list3 = OpcodeList::new();
    list3.push(Opcode {
        id: 65_003,
        spec_version: [1, 4, 0, 0],
        flags: Opcode::FLAG_OPTIONAL,
        parameters: Vec::new(),
    });
    list3.push(Opcode {
        id: opcode_id::WARP_RECTILINEAR,
        spec_version: [1, 3, 0, 0],
        flags: Opcode::FLAG_OPTIONAL,
        parameters: warp_parameters,
    });
    raw_ifd.set(tags::OPCODE_LIST3, Value::Undefined(list3.to_bytes()));
    let rewritten = rewrite.write().expect("opcode fixture rewrite").bytes;
    let frame = DngReader::new()
        .decode_bytes(Path::new("opcodes.dng"), &rewritten, 0)
        .expect("opcode DNG must decode");
    let descriptor = dng_command::descriptor_from_frame(&frame, Path::new("opcodes.dng"))
        .expect("descriptor must serialize opcodes");
    let json = serde_json::to_value(descriptor).expect("descriptor serializes");

    assert_eq!(json["metadata"]["opcodeList1"][0]["id"], 65_001);
    assert_eq!(json["metadata"]["opcodeList1"][0]["parameterLength"], 2);
    assert_eq!(json["metadata"]["opcodeList1"][0]["parametersHex"], "dead");
    assert_eq!(json["metadata"]["opcodeList2"][3]["id"], 9);
    assert_eq!(
        json["metadata"]["opcodeList2"][3]["gainMap"]["points"],
        serde_json::json!([2, 3])
    );
    assert_eq!(
        json["metadata"]["opcodeList2"][3]["gainMap"]["spacing"],
        serde_json::json!([0.5, 0.25])
    );
    assert_eq!(
        json["metadata"]["opcodeList2"][3]["gainMap"]["origin"],
        serde_json::json!([0.1, 0.05])
    );
    assert_eq!(json["metadata"]["opcodeList2"][3]["gainMap"]["planes"], 2);
    assert_eq!(
        json["metadata"]["opcodeList2"][3]["gainMap"]["entries"]
            .as_array()
            .map(Vec::len),
        Some(12)
    );
    assert_eq!(
        json["metadata"]["gainMaps"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(json["metadata"]["opcodeList2"][0]["id"], 65_002);
    assert_eq!(
        json["metadata"]["opcodeList2"][0]["parametersHex"],
        "beef01"
    );
    assert_eq!(json["metadata"]["opcodeList2"][1]["id"], 3);
    assert_eq!(
        json["metadata"]["opcodeList2"][1]["fixVignetteRadial"]["coefficients"],
        serde_json::json!([0.1, 0.02, 0.003, 0.0004, 0.00005])
    );
    assert_eq!(
        json["metadata"]["opcodeList2"][1]["fixVignetteRadial"]["opticalCenter"],
        serde_json::json!([0.45, 0.55])
    );
    assert_eq!(json["metadata"]["opcodeList2"][2]["id"], 3);
    assert_eq!(json["metadata"]["opcodeList2"][2]["flags"], 2);
    assert_eq!(
        json["metadata"]["vignetteRadial"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(json["metadata"]["opcodeList3"][0]["id"], 65_003);
    assert_eq!(json["metadata"]["opcodeList3"][0]["parametersHex"], "");
    assert_eq!(
        json["metadata"]["opcodeList3"][1]["warpRectilinear"]["coefficientSets"][0]["radial"][1],
        0.01
    );
}

#[test]
fn descriptor_serializes_solved_color_reproduce_assets() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S), 3)
        .expect("GH5S frame must decode");
    let descriptor = dng_command::descriptor_from_frame(&frame, Path::new(GH5S))
        .expect("descriptor must preprocess color reproduce");
    let json = serde_json::to_value(&descriptor).expect("descriptor serializes");

    let cr = &json["colorReproduce"];
    assert!(
        cr.is_object(),
        "colorReproduce assets must be present for the GH5S frame"
    );
    // Matrices match the design-doc golden values (1e-4 over JSON f32).
    let sensor = cr["sensorToProphoto"].as_array().expect("matrix present");
    assert_eq!(sensor.len(), 9);
    assert!((sensor[0].as_f64().expect("f64") - 0.759_738).abs() < 1e-4);
    assert!((sensor[4].as_f64().expect("f64") - 1.300_616).abs() < 1e-4);
    let to_srgb = cr["prophotoToSrgb"].as_array().expect("matrix present");
    assert_eq!(to_srgb.len(), 9);
    assert!((to_srgb[0].as_f64().expect("f64") - 2.036_832).abs() < 1e-4);
    // HS calibration: dims (90, 30), interpolated table of 8100 floats
    // (ValueDivs == 1: H x S grid, v layer constant).
    assert_eq!(
        cr["hsDims"]
            .as_array()
            .map(|dims| (dims[0].as_u64(), dims[1].as_u64())),
        Some((Some(90), Some(30)))
    );
    let lut = cr["hsLut"].as_array().expect("interpolated LUT present");
    assert_eq!(lut.len(), 8100);
    assert_eq!(cr["hsEnable"].as_bool(), Some(true));
}
