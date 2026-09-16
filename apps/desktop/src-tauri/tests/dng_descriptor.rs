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
    for (tag, id, parameters) in [
        (tags::OPCODE_LIST1, 65_001, vec![0xde, 0xad]),
        (tags::OPCODE_LIST2, 65_002, vec![0xbe, 0xef, 0x01]),
    ] {
        let mut list = OpcodeList::new();
        list.push(Opcode {
            id,
            spec_version: [1, 4, 0, 0],
            flags: Opcode::FLAG_OPTIONAL | Opcode::FLAG_PREVIEW_SKIP,
            parameters,
        });
        raw_ifd.set(tag, Value::Undefined(list.to_bytes()));
    }
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
    assert_eq!(json["metadata"]["opcodeList2"][0]["id"], 65_002);
    assert_eq!(
        json["metadata"]["opcodeList2"][0]["parametersHex"],
        "beef01"
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
