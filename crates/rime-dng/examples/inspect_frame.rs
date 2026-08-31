use std::path::PathBuf;

use rime_dng::DngReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: inspect_frame <path-to-dng>")?;
    let frame = DngReader::new().decode_file(&path, 0)?;
    println!("path={}", path.display());
    println!("frame_index={}", frame.frame_index);
    println!("dimensions={}x{}", frame.layout.width, frame.layout.height);
    println!("row_stride_samples={}", frame.layout.row_stride_samples);
    println!("storage_bits={}", frame.layout.storage_bits);
    println!("cfa={:?}", frame.layout.cfa);
    println!("dng_version={:?}", frame.metadata.dng_version);
    println!("backward_version={:?}", frame.metadata.backward_version);
    println!("camera_model={}", frame.metadata.camera_model);
    println!("metadata_hash={}", frame.metadata.metadata_hash);
    println!("raw_digest={}", frame.raw_digest);
    println!("sample_count={}", frame.samples().len());
    Ok(())
}
