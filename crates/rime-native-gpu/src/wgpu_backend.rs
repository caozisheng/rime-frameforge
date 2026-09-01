#![expect(
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    reason = "The backend mirrors the fixed v0.1.3 GPU submission contract and narrows validated DNG metadata to GPU f32 parameters."
)]

use std::sync::mpsc;

use bytemuck::{Pod, Zeroable};
use rime_dng::{BayerCfa, DecodedRawFrame, DngReaderError, RawFrameLayout};
use thiserror::Error;

const WGSL: &str = r"
struct FusedParams { width: u32, height: u32, black_level: f32, white_level: f32, cfa_pattern: vec4<u32>, }
@group(0) @binding(0) var raw_input: texture_2d<u32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var<uniform> params: FusedParams;
fn clamp_source(p: vec2<i32>) -> vec2<i32> { return clamp(p, vec2<i32>(0), vec2<i32>(i32(params.width), i32(params.height)) - vec2<i32>(1)); }
fn sample_raw(p: vec2<i32>) -> f32 { return f32(textureLoad(raw_input, clamp_source(p), 0).r); }
fn sample_blc(p: vec2<i32>) -> f32 { return (sample_raw(p) - params.black_level) / (params.white_level - params.black_level); }
fn sample_wbc(p: vec2<i32>) -> f32 { let q = clamp_source(p); let phase = vec2<u32>(u32(q.x) & 1u, u32(q.y) & 1u); let channel = params.cfa_pattern[phase.y * 2u + phase.x]; var gain = 1.0; if (channel == 0u) { gain = 2.0; } if (channel == 2u) { gain = 1.5; } return sample_blc(q) * gain; }
fn cfa(p: vec2<i32>) -> u32 { let q = clamp_source(p); let phase = vec2<u32>(u32(q.x) & 1u, u32(q.y) & 1u); return params.cfa_pattern[phase.y * 2u + phase.x]; }
fn sample_dem(p: vec2<i32>) -> vec3<f32> { let extent = vec2<i32>(i32(params.width), i32(params.height)); let low = max(p - vec2<i32>(1), vec2<i32>(0)); let high = min(p + vec2<i32>(1), extent - vec2<i32>(1)); var sums = vec3<f32>(0.0); var counts = vec3<f32>(0.0); for (var y = low.y; y <= high.y; y++) { for (var x = low.x; x <= high.x; x++) { let q = vec2<i32>(x, y); let channel = cfa(q); sums[channel] += sample_wbc(q); counts[channel] += 1.0; } } return sums / max(counts, vec3<f32>(1.0)); }
fn sample_rgb2yuv(p: vec2<i32>) -> vec4<f32> { let rgb = max(sample_dem(p), vec3<f32>(0.0)); let corrected = vec3<f32>(1.08 * rgb.r - 0.04 * rgb.g - 0.04 * rgb.b, -0.03 * rgb.r + 1.06 * rgb.g - 0.03 * rgb.b, -0.02 * rgb.r - 0.06 * rgb.g + 1.08 * rgb.b); let encoded = pow(max(corrected, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)); return vec4<f32>(dot(encoded, vec3<f32>(0.2126, 0.7152, 0.0722)), dot(encoded, vec3<f32>(-0.114572, -0.385428, 0.5)) + 0.5, dot(encoded, vec3<f32>(0.5, -0.454153, -0.045847)) + 0.5, 1.0); }
@compute @workgroup_size(8, 8)
fn normal_fused_main(@builtin(global_invocation_id) gid: vec3<u32>) { if (gid.x >= params.width || gid.y >= params.height) { return; } textureStore(output_texture, vec2<i32>(gid.xy), sample_rgb2yuv(vec2<i32>(gid.xy))); }
";

#[derive(Debug, Error)]
pub enum WgpuReadbackError {
    #[error("GPU adapter is unavailable")]
    AdapterUnavailable,
    #[error("GPU device creation failed: {0}")]
    Device(String),
    #[error("GPU readback failed: {0}")]
    Readback(String),
    #[error("GPU input is invalid: {0}")]
    Input(#[from] DngReaderError),
    #[error("native graph error: {0}")]
    Graph(#[from] super::NativePipelineError),
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FusedParams {
    width: u32,
    height: u32,
    black_level: f32,
    white_level: f32,
    cfa_pattern: [u32; 4],
}

pub struct WgpuReadbackExecutor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

impl WgpuReadbackExecutor {
    pub fn new() -> Result<Self, WgpuReadbackError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .map_err(|_| WgpuReadbackError::AdapterUnavailable)?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|error| WgpuReadbackError::Device(error.to_string()))?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rime-native-normal-fused"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("rime-native-normal-fused"),
            layout: None,
            module: &module,
            entry_point: Some("normal_fused_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Ok(Self {
            device,
            queue,
            pipeline,
        })
    }

    #[must_use]
    pub const fn backend(&self) -> super::NativeGpuBackend {
        super::NativeGpuBackend::WgpuReadback
    }

    pub fn render(
        &self,
        frame: &DecodedRawFrame,
    ) -> Result<super::PreviewSurface, WgpuReadbackError> {
        self.render_with_identity(
            frame,
            super::NativeFrameIdentity {
                frame_index: frame.frame_index,
                run_revision: 0,
                method_revision: 0,
                gpu_generation: 0,
                phase: rime_core::FramePhase::Output,
            },
        )
    }

    pub fn render_with_identity(
        &self,
        frame: &DecodedRawFrame,
        identity: super::NativeFrameIdentity,
    ) -> Result<super::PreviewSurface, WgpuReadbackError> {
        Self::validate_input(&frame.layout, frame.samples().len())?;
        let width = frame.layout.width;
        let height = frame.layout.height;
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rime-native-raw"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Uint,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rime-native-output"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rime-native-fused-params"),
            size: std::mem::size_of::<FusedParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cfa = Self::cfa_pattern(frame.layout.cfa).ok_or(DngReaderError::UnsupportedCfa)?;
        let data = FusedParams {
            width,
            height,
            black_level: frame.metadata.black_levels.first().copied().unwrap_or(0.0) as f32,
            white_level: frame
                .metadata
                .white_levels
                .first()
                .copied()
                .unwrap_or(4095.0) as f32,
            cfa_pattern: cfa,
        };
        self.queue
            .write_buffer(&params, 0, bytemuck::bytes_of(&data));
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(frame.samples()),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.layout.row_stride_samples * 2),
                rows_per_image: Some(height),
            },
            extent,
        );
        let row_bytes = super::aligned_readback_bytes_per_row(width);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rime-native-readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let input_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rime-native-fused-bind-group"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rime-native-frame"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("rime-native-fused-compute"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| WgpuReadbackError::Readback(error.to_string()))?;
        receiver
            .recv()
            .map_err(|error| WgpuReadbackError::Readback(error.to_string()))?
            .map_err(|error| WgpuReadbackError::Readback(error.to_string()))?;
        let mapped = slice.get_mapped_range();
        let visible = width * 16;
        let pixels = mapped
            .chunks_exact(row_bytes as usize)
            .flat_map(|row| {
                bytemuck::cast_slice::<u8, f32>(&row[..visible as usize])
                    .iter()
                    .copied()
            })
            .collect();
        drop(mapped);
        readback.unmap();
        super::PreviewSurface::new(identity, width, height, pixels)
            .map_err(WgpuReadbackError::Graph)
    }

    pub fn validate_input(
        layout: &RawFrameLayout,
        sample_count: usize,
    ) -> Result<(), WgpuReadbackError> {
        rime_dng::DngReader::validate_layout(layout)?;
        let expected = usize::try_from(layout.row_stride_samples)
            .ok()
            .and_then(|stride| {
                usize::try_from(layout.height)
                    .ok()
                    .and_then(|height| stride.checked_mul(height))
            })
            .ok_or(DngReaderError::SampleCountMismatch)?;
        if expected != sample_count {
            return Err(WgpuReadbackError::Input(
                DngReaderError::SampleCountMismatch,
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn cfa_pattern(cfa: BayerCfa) -> Option<[u32; 4]> {
        match cfa {
            BayerCfa::Rggb => Some([0, 1, 1, 2]),
            BayerCfa::Grbg => Some([1, 0, 2, 1]),
            BayerCfa::Gbrg => Some([1, 2, 0, 1]),
            BayerCfa::Bggr => Some([2, 1, 1, 0]),
            BayerCfa::Unsupported => None,
        }
    }
}
