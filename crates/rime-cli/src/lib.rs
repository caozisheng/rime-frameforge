use std::{
    cmp::Ordering,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Stdio},
};

use clap::{Args, Parser, Subcommand};
use image::{ImageBuffer, Rgba};
use rime_core::GraphQuantizationConfig;
use rime_dng::DngReader;
use rime_native_gpu::{
    BoundedFrameRing, FrameSlotState, NativePipelineConfig, WgpuReadbackError,
    WgpuReadbackExecutor, build_normal_graph_plan,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(name = "rime-frameforge")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn try_parse_from<I, T>(iter: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(iter)
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Inspect {
        input: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Render {
        input: PathBuf,
        #[command(flatten)]
        options: RenderOptions,
    },
    RenderSequence {
        input: PathBuf,
        #[command(flatten)]
        options: RenderOptions,
    },
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum GraphCommand {
    Validate {
        #[arg(long, default_value = "normal")]
        graph: String,
    },
    Show,
}

#[derive(Clone, Debug, Args)]
pub struct RenderOptions {
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub quiet: bool,
    #[arg(long, default_value = "jsonl")]
    pub progress: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub print_resolved_config: bool,
    #[arg(long)]
    pub graph_config: Option<PathBuf>,
    #[arg(long, default_value = "h264")]
    pub codec: String,
    #[arg(long, default_value_t = 24)]
    pub fps: u32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            output: PathBuf::new(),
            json: false,
            quiet: false,
            progress: Some("jsonl".to_owned()),
            dry_run: false,
            print_resolved_config: false,
            graph_config: None,
            codec: "h264".to_owned(),
            fps: 24,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedGraphConfig {
    pub graph_id: String,
    pub manifest_hash: String,
    pub ring_capacity: usize,
    pub encoder_backend: &'static str,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("CLI_ARGUMENT_INVALID: {0}")]
    Argument(String),
    #[error("CLI_IO_FAILED: {0}")]
    Io(#[from] std::io::Error),
    #[error("CLI_DNG_FAILED: {0}")]
    Dng(#[from] rime_dng::DngReaderError),
    #[error("CLI_GPU_FAILED: {0}")]
    Gpu(#[from] WgpuReadbackError),
    #[error("CLI_GRAPH_INVALID: {0}")]
    Graph(String),
    #[error("CLI_IMAGE_FAILED: {0}")]
    Image(#[from] image::ImageError),
    #[error("CLI_JSON_FAILED: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CLI_FFMPEG_FAILED: {0}")]
    Ffmpeg(String),
}

pub fn resolve_graph_config(path: Option<&Path>) -> Result<ResolvedGraphConfig, CliError> {
    let plan = build_normal_graph_plan().map_err(|error| CliError::Graph(error.to_string()))?;
    if let Some(path) = path {
        let config: GraphQuantizationConfig = serde_json::from_slice(&fs::read(path)?)?;
        let defaults =
            GraphQuantizationConfig::defaults_for(&rime_isp::build_normal_graph_presentation())
                .map_err(|error| CliError::Graph(error.to_string()))?;
        if config != defaults {
            return Err(CliError::Argument("NATIVE_CONFIG_UNSUPPORTED: native readback currently supports only v0.1.3 default Normal Graph quantization".to_owned()));
        }
    }
    Ok(ResolvedGraphConfig {
        graph_id: plan.graph_id().to_owned(),
        manifest_hash: plan.manifest_hash().to_owned(),
        ring_capacity: NativePipelineConfig::default().ring_capacity,
        encoder_backend: "cpu_readback",
    })
}

pub fn scan_dng_sequence(selected: &Path) -> Result<Vec<PathBuf>, CliError> {
    let parent = selected
        .parent()
        .ok_or_else(|| CliError::Argument("selected DNG has no parent directory".to_owned()))?;
    let mut paths = fs::read_dir(parent)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_file())
                .and_then(|_| {
                    path.extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("dng"))
                        .then_some(path)
                })
        })
        .collect::<Vec<_>>();
    natural_sort_dng_paths(&mut paths, selected);
    if paths.is_empty() {
        return Err(CliError::Argument(
            "no DNG files found in selected parent directory".to_owned(),
        ));
    }
    Ok(paths)
}

pub fn natural_sort_dng_paths(paths: &mut Vec<PathBuf>, selected: &Path) {
    let parent = selected.parent();
    paths.retain(|path| {
        parent.is_some_and(|directory| path.parent() == Some(directory))
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dng"))
    });
    paths.sort_by(|left, right| natural_path_cmp(left, right));
}

pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Inspect { input, json } => inspect(&input, json),
        Command::Render { input, options } => render(&input, &options),
        Command::RenderSequence { input, options } => render_sequence(&input, &options),
        Command::Graph {
            command: GraphCommand::Validate { graph },
        } => {
            if graph != "normal" {
                return Err(CliError::Argument(format!("unsupported graph {graph}")));
            }
            println!("{}", serde_json::to_string(&resolve_graph_config(None)?)?);
            Ok(())
        }
        Command::Graph {
            command: GraphCommand::Show,
        } => {
            println!(
                "{}",
                rime_isp::render_normal_manifest_json()
                    .map_err(|error| CliError::Graph(error.to_string()))?
            );
            Ok(())
        }
    }
}

fn inspect(input: &Path, json: bool) -> Result<(), CliError> {
    let frame = DngReader::new().decode_file(input, 0)?;
    let result = serde_json::json!({
        "path": input, "frame_index": frame.frame_index, "width": frame.layout.width,
        "height": frame.layout.height, "storage_bits": frame.layout.storage_bits,
        "cfa": format!("{:?}", frame.layout.cfa).to_lowercase(),
        "camera_model": frame.metadata.camera_model, "metadata_hash": frame.metadata.metadata_hash,
        "raw_digest": frame.raw_digest,
    });
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!(
            "{}x{} {:?}",
            frame.layout.width, frame.layout.height, frame.layout.cfa
        );
    }
    Ok(())
}

fn render(input: &Path, options: &RenderOptions) -> Result<(), CliError> {
    let config = resolve_graph_config(options.graph_config.as_deref())?;
    print_resolved(options, &config)?;
    if options.dry_run {
        return Ok(());
    }
    let frame = DngReader::new().decode_file(input, 0)?;
    let surface = WgpuReadbackExecutor::new()?.render(&frame)?;
    write_png(
        &options.output,
        surface.width(),
        surface.height(),
        surface.pixels(),
    )?;
    emit(
        options,
        serde_json::json!({"event":"completed", "frames":1, "output":options.output, "encoder_backend":config.encoder_backend}),
    )
}

fn render_sequence(input: &Path, options: &RenderOptions) -> Result<(), CliError> {
    let config = resolve_graph_config(options.graph_config.as_deref())?;
    print_resolved(options, &config)?;
    let paths = scan_dng_sequence(input)?;
    if options.dry_run {
        return emit(
            options,
            serde_json::json!({"event":"dry_run", "frames":paths.len(), "output":options.output, "encoder_backend":config.encoder_backend}),
        );
    }
    let executor = WgpuReadbackExecutor::new()?;
    let mut ring = BoundedFrameRing::new(config.ring_capacity)
        .map_err(|error| CliError::Graph(error.to_string()))?;
    let mut encoder: Option<Child> = None;
    let mut dimensions = None;
    for (index, path) in paths.iter().enumerate() {
        let slot = ring
            .claim_empty()
            .ok_or_else(|| CliError::Graph("bounded frame ring exhausted".to_owned()))?;
        ring.transition(slot, FrameSlotState::Decoding)
            .map_err(|error| CliError::Graph(error.to_string()))?;
        emit(
            options,
            serde_json::json!({"event":"frame_started", "index":index, "total":paths.len()}),
        )?;
        let frame = DngReader::new().decode_file(path, index as u64)?;
        ring.transition(slot, FrameSlotState::Decoded)
            .map_err(|error| CliError::Graph(error.to_string()))?;
        let surface = executor.render(&frame)?;
        ring.transition(slot, FrameSlotState::GpuSubmitted)
            .map_err(|error| CliError::Graph(error.to_string()))?;
        let extent = (surface.width(), surface.height());
        if let Some(expected) = dimensions {
            if expected != extent {
                return Err(CliError::Argument(
                    "sequence frame dimensions differ".to_owned(),
                ));
            }
        } else {
            encoder = Some(ffmpeg_encoder(
                &options.output,
                options.fps,
                &options.codec,
                extent.0,
                extent.1,
            )?);
            dimensions = Some(extent);
        }
        encoder
            .as_mut()
            .and_then(|child| child.stdin.as_mut())
            .ok_or_else(|| CliError::Ffmpeg("encoder stdin unavailable".to_owned()))?
            .write_all(&yuv_to_rgb8(surface.pixels()))?;
        ring.transition(slot, FrameSlotState::Encoded)
            .map_err(|error| CliError::Graph(error.to_string()))?;
        ring.transition(slot, FrameSlotState::Reusable)
            .map_err(|error| CliError::Graph(error.to_string()))?;
        emit(
            options,
            serde_json::json!({"event":"frame_completed", "index":index}),
        )?;
    }
    let status = encoder
        .ok_or_else(|| CliError::Argument("sequence is empty".to_owned()))?
        .wait()?;
    if !status.success() {
        return Err(CliError::Ffmpeg(format!("ffmpeg exited with {status}")));
    }
    emit(
        options,
        serde_json::json!({"event":"completed", "frames":paths.len(), "output":options.output, "encoder_backend":config.encoder_backend}),
    )
}

fn ffmpeg_encoder(
    output: &Path,
    fps: u32,
    codec: &str,
    width: u32,
    height: u32,
) -> Result<Child, CliError> {
    let codec_name = match codec {
        "h264" => "libx264",
        "hevc" => "libx265",
        _ => return Err(CliError::Argument(format!("unsupported codec {codec}"))),
    };
    ProcessCommand::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            &format!("{width}x{height}"),
            "-framerate",
            &fps.to_string(),
            "-i",
            "-",
            "-c:v",
            codec_name,
        ])
        .arg(output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(CliError::Io)
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[f32]) -> Result<(), CliError> {
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, yuv_to_rgba8(pixels))
        .ok_or_else(|| {
            CliError::Argument("PNG dimensions do not match native surface".to_owned())
        })?;
    image.save(path)?;
    Ok(())
}

fn yuv_to_rgba8(pixels: &[f32]) -> Vec<u8> {
    pixels
        .chunks_exact(4)
        .flat_map(|pixel| {
            let u = pixel[1] - 0.5;
            let v = pixel[2] - 0.5;
            let rgb = [
                pixel[0] + 1.5748 * v,
                pixel[0] - 0.187_324 * u - 0.468_124 * v,
                pixel[0] + 1.8556 * u,
            ];
            rgb.into_iter()
                .map(|value| (value.clamp(0.0, 1.0) * 255.0).trunc() as u8)
                .chain(std::iter::once(255))
                .collect::<Vec<_>>()
        })
        .collect()
}
fn yuv_to_rgb8(pixels: &[f32]) -> Vec<u8> {
    yuv_to_rgba8(pixels)
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect()
}
fn print_resolved(options: &RenderOptions, config: &ResolvedGraphConfig) -> Result<(), CliError> {
    if options.print_resolved_config {
        println!("{}", serde_json::to_string(config)?);
    }
    Ok(())
}
fn emit(options: &RenderOptions, event: serde_json::Value) -> Result<(), CliError> {
    if !options.quiet && (options.json || options.progress.as_deref() == Some("jsonl")) {
        println!("{}", serde_json::to_string(&event)?);
    }
    Ok(())
}

fn natural_path_cmp(left: &Path, right: &Path) -> Ordering {
    natural_str_cmp(
        &left
            .file_name()
            .map_or_else(|| left.to_string_lossy(), |name| name.to_string_lossy()),
        &right
            .file_name()
            .map_or_else(|| right.to_string_lossy(), |name| name.to_string_lossy()),
    )
}
fn natural_str_cmp(left: &str, right: &str) -> Ordering {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    let (mut l, mut r) = (0, 0);
    while l < left.len() && r < right.len() {
        if left[l].is_ascii_digit() && right[r].is_ascii_digit() {
            let le = digit_run_end(left, l);
            let re = digit_run_end(right, r);
            let ls = significant_digits(left, l, le);
            let rs = significant_digits(right, r, re);
            let ordering = ls
                .len()
                .cmp(&rs.len())
                .then_with(|| ls.cmp(rs))
                .then_with(|| (le - l).cmp(&(re - r)));
            if ordering != Ordering::Equal {
                return ordering;
            }
            l = le;
            r = re;
        } else {
            let ordering = left[l]
                .to_ascii_lowercase()
                .cmp(&right[r].to_ascii_lowercase());
            if ordering != Ordering::Equal {
                return ordering;
            }
            l += 1;
            r += 1;
        }
    }
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}
fn digit_run_end(value: &[u8], mut index: usize) -> usize {
    while index < value.len() && value[index].is_ascii_digit() {
        index += 1;
    }
    index
}
fn significant_digits(value: &[u8], mut start: usize, end: usize) -> &[u8] {
    while start < end && value[start] == b'0' {
        start += 1;
    }
    &value[start..end]
}
