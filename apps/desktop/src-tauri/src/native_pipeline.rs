use std::{io::Cursor, path::Path, sync::Mutex, time::Instant};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde::Serialize;
use tauri::{Emitter, State};

pub struct NativePipelineState {
    pub executor: Mutex<Option<rime_native_gpu::WgpuReadbackExecutor>>,
}

impl Default for NativePipelineState {
    fn default() -> Self {
        Self {
            executor: Mutex::new(None),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRenderDescriptor {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub preview_width: u32,
    pub preview_height: u32,
    pub node_id: &'static str,
    pub port_id: &'static str,
    pub encoder_backend: &'static str,
    pub preview_data_url: String,
}

#[tauri::command]
pub fn inspect_dng_native(
    path: String,
    frame_index: Option<u64>,
) -> Result<crate::dng_command::DngFrameDescriptor, String> {
    let path = Path::new(&path);
    let frame = rime_dng::DngReader::new()
        .decode_file(path, frame_index.unwrap_or(0))
        .map_err(|error| format!("NATIVE_DNG_DECODE_FAILED: {error}"))?;
    crate::dng_command::descriptor_from_frame(&frame, path)
}

#[tauri::command]
pub fn render_dng_native(
    app: tauri::AppHandle,
    state: State<'_, NativePipelineState>,
    path: String,
    frame_index: Option<u64>,
) -> Result<NativeRenderDescriptor, String> {
    let frame_started = Instant::now();
    let index = frame_index.unwrap_or(0);
    emit(
        &app,
        NativePipelineEvent::FrameStarted { frame_index: index },
    )?;
    let frame = rime_dng::DngReader::new()
        .decode_file(Path::new(&path), index)
        .map_err(|error| format!("NATIVE_DNG_DECODE_FAILED: {error}"))?;
    let decode_elapsed = frame_started.elapsed();
    let mut executor = state
        .executor
        .lock()
        .map_err(|_| "NATIVE_GPU_STATE_POISONED".to_owned())?;
    let initialize_started = Instant::now();
    if executor.is_none() {
        *executor = Some(
            rime_native_gpu::WgpuReadbackExecutor::new()
                .map_err(|error| format!("NATIVE_GPU_INIT_FAILED: {error}"))?,
        );
    }
    let initialize_elapsed = initialize_started.elapsed();
    let render_started = Instant::now();
    let surface = executor
        .as_ref()
        .ok_or_else(|| "NATIVE_GPU_EXECUTOR_MISSING".to_owned())?
        .render(&frame)
        .map_err(|error| format!("NATIVE_GPU_RENDER_FAILED: {error}"))?;
    let render_elapsed = render_started.elapsed();
    let preview_started = Instant::now();
    let (preview_data_url, preview_width, preview_height) =
        encode_preview(surface.width(), surface.height(), surface.pixels())?;
    let preview_elapsed = preview_started.elapsed();
    let descriptor = NativeRenderDescriptor {
        frame_index: surface.identity().frame_index,
        width: surface.width(),
        height: surface.height(),
        preview_width,
        preview_height,
        node_id: surface.node_id(),
        port_id: surface.port_id(),
        encoder_backend: "cpu_readback",
        preview_data_url,
    };
    eprintln!(
        "native frame {index}: decode={:.2}ms init={:.2}ms gpu_readback={:.2}ms preview={:.2}ms total={:.2}ms",
        decode_elapsed.as_secs_f64() * 1000.0,
        initialize_elapsed.as_secs_f64() * 1000.0,
        render_elapsed.as_secs_f64() * 1000.0,
        preview_elapsed.as_secs_f64() * 1000.0,
        frame_started.elapsed().as_secs_f64() * 1000.0,
    );
    emit(
        &app,
        NativePipelineEvent::FrameCompleted {
            descriptor: &descriptor,
        },
    )?;
    Ok(descriptor)
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum NativePipelineEvent<'a> {
    FrameStarted {
        frame_index: u64,
    },
    FrameCompleted {
        descriptor: &'a NativeRenderDescriptor,
    },
}

fn emit(app: &tauri::AppHandle, event: NativePipelineEvent<'_>) -> Result<(), String> {
    app.emit("native-pipeline", event)
        .map_err(|error| format!("NATIVE_PIPELINE_EVENT_FAILED: {error}"))
}

fn encode_preview(width: u32, height: u32, pixels: &[f32]) -> Result<(String, u32, u32), String> {
    let scale = (width.max(height) as f32 / 640.0).max(1.0).ceil() as u32;
    let preview_width = width.div_ceil(scale);
    let preview_height = height.div_ceil(scale);
    let mut rgba = Vec::with_capacity((preview_width * preview_height * 4) as usize);
    for y in 0..preview_height {
        for x in 0..preview_width {
            let source_x = (x * scale).min(width - 1);
            let source_y = (y * scale).min(height - 1);
            let pixel = &pixels[((source_y * width + source_x) * 4) as usize..];
            let u = pixel[1] - 0.5;
            let v = pixel[2] - 0.5;
            for value in [
                pixel[0] + 1.5748 * v,
                pixel[0] - 0.187_324 * u - 0.468_124 * v,
                pixel[0] + 1.8556 * u,
            ] {
                rgba.push((value.clamp(0.0, 1.0) * 255.0).trunc() as u8);
            }
            rgba.push(255);
        }
    }
    let mut bytes = Cursor::new(Vec::new());
    PngEncoder::new(&mut bytes)
        .write_image(
            &rgba,
            preview_width,
            preview_height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("NATIVE_PREVIEW_ENCODE_FAILED: {error}"))?;
    Ok((
        format!(
            "data:image/png;base64,{}",
            BASE64.encode(bytes.into_inner())
        ),
        preview_width,
        preview_height,
    ))
}

#[cfg(test)]
mod tests {
    use super::encode_preview;

    #[test]
    fn native_preview_is_a_bounded_png_data_url() {
        let pixels = vec![0.5, 0.5, 0.5, 1.0];
        let (preview, width, height) = encode_preview(1, 1, &pixels).expect("preview encodes");

        assert!(preview.starts_with("data:image/png;base64,"));
        assert_eq!((width, height), (1, 1));
        assert!(preview.len() < 256);
    }
}
