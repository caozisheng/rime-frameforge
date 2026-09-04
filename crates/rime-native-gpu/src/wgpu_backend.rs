#![expect(
    clippy::missing_errors_doc,
    clippy::cast_possible_truncation,
    reason = "The backend mirrors the explicit graph submission contract and narrows validated DNG metadata to GPU f32 parameters."
)]

use std::sync::{Mutex, mpsc};

use rime_core::{ResourceFormat, SignalDomain};
use rime_dng::{BayerCfa, DecodedRawFrame, DngReaderError, RawFrameLayout};
use rime_isp::{
    FrameIdentity, ModuleParameterPacket, Operator, OperatorError, PreprocessContext, ShaderAsset,
};
use thiserror::Error;
use wgpu::util::DeviceExt as _;

#[derive(Debug, Error)]
pub enum WgpuReadbackError {
    #[error("GPU adapter is unavailable")]
    AdapterUnavailable,
    #[error("GPU device creation failed: {0}")]
    Device(String),
    #[error("GPU readback failed: {0}")]
    Readback(String),
    #[error("GPU graph resource failed: {0}")]
    Resource(String),
    #[error("GPU input is invalid: {0}")]
    Input(#[from] DngReaderError),
    #[error("ISP operator failed: {0}")]
    Operator(#[from] OperatorError),
    #[error("native graph error: {0}")]
    Graph(#[from] super::NativePipelineError),
}

struct CompiledOperator {
    operator: &'static dyn Operator,
    shader: &'static ShaderAsset,
    pipeline: wgpu::ComputePipeline,
}

struct PooledTexture {
    domain: SignalDomain,
    format: ResourceFormat,
    width: u32,
    height: u32,
    texture: wgpu::Texture,
}

pub struct WgpuReadbackExecutor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    operators: Vec<CompiledOperator>,
    texture_pool: Mutex<Vec<PooledTexture>>,
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
        let mut operators = Vec::new();
        for operator in rime_isp::normal_operators().iter().copied() {
            let definition = operator.definition();
            for method in definition.methods {
                let shader = &method.shader;
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(definition.id),
                    source: wgpu::ShaderSource::Wgsl(shader.source.into()),
                });
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(definition.id),
                    layout: None,
                    module: &module,
                    entry_point: Some(shader.entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });
                operators.push(CompiledOperator {
                    operator,
                    shader,
                    pipeline,
                });
            }
        }
        Ok(Self {
            device,
            queue,
            operators,
            texture_pool: Mutex::new(Vec::new()),
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
        let cfa_pattern =
            Self::cfa_pattern(frame.layout.cfa).ok_or(DngReaderError::UnsupportedCfa)?;
        let preprocess_context = PreprocessContext {
            identity: FrameIdentity {
                frame_index: identity.frame_index,
                run_revision: identity.run_revision,
                method_revision: identity.method_revision,
            },
            width,
            height,
            black_level: frame.metadata.black_levels.first().copied().unwrap_or(0.0) as f32,
            white_level: frame
                .metadata
                .white_levels
                .first()
                .copied()
                .unwrap_or(4095.0) as f32,
            cfa_pattern,
            as_shot_neutral: frame.metadata.as_shot_neutral,
            as_shot_white_xy: frame.metadata.as_shot_white_xy,
            color_matrix1: frame.metadata.color_matrix1,
            color_matrix2: frame.metadata.color_matrix2,
        };
        let plan = super::build_normal_graph_plan()?;
        let order = plan
            .execution_order()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let raw_texture = self.upload_raw(frame);
        let mut current: Option<PooledTexture> = None;
        super::execute_operator_phases(&order, &preprocess_context, |operator, packet| {
            let compiled = self
                .operators
                .iter()
                .find(|compiled| {
                    std::ptr::eq(compiled.operator, operator)
                        && compiled.shader.method == packet.method()
                })
                .ok_or(OperatorError::Preprocess {
                    module_id: operator.definition().id,
                    reason: "operator method has no compiled GPU pipeline",
                })?;
            let input = current
                .as_ref()
                .map_or(&raw_texture, |resource| &resource.texture);
            let output = self
                .acquire_texture(
                    compiled.operator.definition().output.domain,
                    compiled.operator.definition().output.format,
                    width,
                    height,
                )
                .map_err(|_| OperatorError::Preprocess {
                    module_id: operator.definition().id,
                    reason: "texture pool is unavailable",
                })?;
            self.dispatch(compiled, packet, input, &output.texture)
                .map_err(|_| OperatorError::Preprocess {
                    module_id: operator.definition().id,
                    reason: "GPU dispatch failed",
                })?;
            if let Some(previous) = current.replace(output) {
                self.release_texture(previous)
                    .map_err(|_| OperatorError::Preprocess {
                        module_id: operator.definition().id,
                        reason: "texture pool is unavailable",
                    })?;
            }
            Ok(())
        })?;
        let final_output = current.ok_or_else(|| {
            WgpuReadbackError::Resource("normal graph produced no output texture".to_owned())
        })?;
        let pixels = self.readback_rgba(&final_output.texture, width, height)?;
        self.release_texture(final_output)?;
        super::PreviewSurface::new(identity, width, height, pixels)
            .map_err(WgpuReadbackError::Graph)
    }

    fn upload_raw(&self, frame: &DecodedRawFrame) -> wgpu::Texture {
        let extent = wgpu::Extent3d {
            width: frame.layout.width,
            height: frame.layout.height,
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
                rows_per_image: Some(frame.layout.height),
            },
            extent,
        );
        texture
    }

    fn dispatch(
        &self,
        compiled: &CompiledOperator,
        packet: &ModuleParameterPacket,
        input: &wgpu::Texture,
        output: &wgpu::Texture,
    ) -> Result<(), WgpuReadbackError> {
        let input_view = input.create_view(&wgpu::TextureViewDescriptor::default());
        let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let uniform = match (compiled.shader.bindings.uniform, packet.bytes().is_empty()) {
            (Some(_), false) => Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(packet.module_id()),
                    contents: packet.bytes(),
                    usage: wgpu::BufferUsages::UNIFORM,
                },
            )),
            (Some(_), true) => {
                return Err(WgpuReadbackError::Resource(format!(
                    "operator `{}` requires a uniform packet",
                    packet.module_id()
                )));
            }
            (None, false) => {
                return Err(WgpuReadbackError::Resource(format!(
                    "operator `{}` emitted an undeclared uniform packet",
                    packet.module_id()
                )));
            }
            (None, true) => None,
        };
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: compiled.shader.bindings.input,
                resource: wgpu::BindingResource::TextureView(&input_view),
            },
            wgpu::BindGroupEntry {
                binding: compiled.shader.bindings.output,
                resource: wgpu::BindingResource::TextureView(&output_view),
            },
        ];
        if let (Some(binding), Some(buffer)) = (compiled.shader.bindings.uniform, uniform.as_ref())
        {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            });
        }
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(packet.module_id()),
            layout: &compiled.pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(packet.module_id()),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(packet.module_id()),
                timestamp_writes: None,
            });
            pass.set_pipeline(&compiled.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let [x, y, z] = compiled.shader.workgroup_size;
            pass.dispatch_workgroups(output.width().div_ceil(x), output.height().div_ceil(y), z);
        }
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| WgpuReadbackError::Resource(error.to_string()))?;
        Ok(())
    }

    fn acquire_texture(
        &self,
        domain: SignalDomain,
        format: ResourceFormat,
        width: u32,
        height: u32,
    ) -> Result<PooledTexture, WgpuReadbackError> {
        let mut pool = self
            .texture_pool
            .lock()
            .map_err(|error| WgpuReadbackError::Resource(error.to_string()))?;
        if let Some(index) = pool.iter().position(|resource| {
            resource.domain == domain
                && resource.format == format
                && resource.width == width
                && resource.height == height
        }) {
            return Ok(pool.swap_remove(index));
        }
        drop(pool);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rime-native-operator-output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format(format),
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(PooledTexture {
            domain,
            format,
            width,
            height,
            texture,
        })
    }

    fn release_texture(&self, texture: PooledTexture) -> Result<(), WgpuReadbackError> {
        self.texture_pool
            .lock()
            .map_err(|error| WgpuReadbackError::Resource(error.to_string()))?
            .push(texture);
        Ok(())
    }

    fn readback_rgba(
        &self,
        output: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<Vec<f32>, WgpuReadbackError> {
        let row_bytes = super::aligned_readback_bytes_per_row(width);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rime-native-readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rime-native-preview-readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: output,
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
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
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
        Ok(pixels)
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
    pub const fn cfa_pattern(cfa: BayerCfa) -> Option<[u32; 4]> {
        match cfa {
            BayerCfa::Rggb => Some([0, 1, 1, 2]),
            BayerCfa::Grbg => Some([1, 0, 2, 1]),
            BayerCfa::Gbrg => Some([1, 2, 0, 1]),
            BayerCfa::Bggr => Some([2, 1, 1, 0]),
            BayerCfa::Unsupported => None,
        }
    }
}

fn texture_format(format: ResourceFormat) -> wgpu::TextureFormat {
    match format {
        ResourceFormat::R16Uint => wgpu::TextureFormat::R16Uint,
        ResourceFormat::R32Float => wgpu::TextureFormat::R32Float,
        ResourceFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
    }
}
