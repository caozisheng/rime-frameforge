#![expect(
    clippy::cast_possible_truncation,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    reason = "the backend mirrors the explicit graph submission contract and narrows validated DNG/image coordinates to GPU resource domains"
)]

use rime_core::{ResourceFormat, SignalDomain};
use rime_dng::{BayerCfa, DecodedRawFrame, DngReaderError, RawFrameLayout};
use rime_isp::vbe::drc::{DrcExposurePolicy, build_bayer_local_statistics};
use rime_isp::{
    FrameIdentity, ModuleParameterPacket, Operator, OperatorError, PreprocessContext, ShaderAsset,
};
use std::sync::{Mutex, mpsc};
use thiserror::Error;
use wgpu::util::DeviceExt as _;

const MAX_READBACK_BYTES: u64 = 128 * 1024 * 1024;
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

struct DrcPipelines {
    prefilter: wgpu::ComputePipeline,
    downsample: wgpu::ComputePipeline,
    reconstruct: wgpu::ComputePipeline,
    guided_coefficients: wgpu::ComputePipeline,
    guided_apply_vertical: wgpu::ComputePipeline,
    combine_global: wgpu::ComputePipeline,
    combine_local: wgpu::ComputePipeline,
}

impl DrcPipelines {
    fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("drc-pipeline"),
            source: wgpu::ShaderSource::Wgsl(rime_isp::vbe::drc::DRC_PIPELINE_WGSL.into()),
        });
        let pipeline = |entry_point: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: None,
                module: &module,
                entry_point: Some(entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        Self {
            prefilter: pipeline("drc_prefilter_main"),
            downsample: pipeline("pyramid_downsample_main"),
            reconstruct: pipeline("pyramid_reconstruct_main"),
            guided_coefficients: pipeline("guided_coefficients_main"),
            guided_apply_vertical: pipeline("guided_apply_vertical_main"),
            combine_global: pipeline("drc_combine_global_main"),
            combine_local: pipeline("drc_combine_local_main"),
        }
    }
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
    drc_pipelines: DrcPipelines,
}

/// Per-render DRC/WBC options threaded through `render_internal`.
#[derive(Clone, Copy)]
struct RenderSetup<'a> {
    drc_method: Option<&'a str>,
    drc_exposure_policy: DrcExposurePolicy,
    drc_metered_target_ev100: Option<f64>,
    drc_profile_adjustment_ev: f64,
    wbc_highlight_recovery: bool,
    drc_details_amplify: bool,
}

/// Runtime ISP feature switches shared by the WBC and DRC modules.
#[derive(Clone, Copy)]
pub struct RenderFeatureFlags {
    pub wbc_highlight_recovery: bool,
    pub drc_details_amplify: bool,
}

impl Default for RenderFeatureFlags {
    fn default() -> Self {
        Self {
            wbc_highlight_recovery: false,
            drc_details_amplify: true,
        }
    }
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
        let drc_pipelines = DrcPipelines::new(&device);
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
            drc_pipelines,
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

    pub fn render_with_drc_method(
        &self,
        frame: &DecodedRawFrame,
        method: &str,
    ) -> Result<super::PreviewSurface, WgpuReadbackError> {
        self.render_with_drc_options(
            frame,
            method,
            DrcExposurePolicy::Baseline,
            None,
            0.0,
            RenderFeatureFlags::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_with_drc_options(
        &self,
        frame: &DecodedRawFrame,
        method: &str,
        exposure_policy: DrcExposurePolicy,
        metered_target_ev100: Option<f64>,
        profile_adjustment_ev: f64,
        features: RenderFeatureFlags,
    ) -> Result<super::PreviewSurface, WgpuReadbackError> {
        self.render_internal(
            frame,
            super::NativeFrameIdentity {
                frame_index: frame.frame_index,
                run_revision: 0,
                method_revision: 0,
                gpu_generation: 0,
                phase: rime_core::FramePhase::Output,
            },
            &RenderSetup {
                drc_method: Some(method),
                drc_exposure_policy: exposure_policy,
                drc_metered_target_ev100: metered_target_ev100,
                drc_profile_adjustment_ev: profile_adjustment_ev,
                wbc_highlight_recovery: features.wbc_highlight_recovery,
                drc_details_amplify: features.drc_details_amplify,
            },
        )
    }

    pub fn render_with_identity(
        &self,
        frame: &DecodedRawFrame,
        identity: super::NativeFrameIdentity,
    ) -> Result<super::PreviewSurface, WgpuReadbackError> {
        self.render_with_identity_options(frame, identity, RenderFeatureFlags::default())
    }

    pub fn render_with_identity_options(
        &self,
        frame: &DecodedRawFrame,
        identity: super::NativeFrameIdentity,
        features: RenderFeatureFlags,
    ) -> Result<super::PreviewSurface, WgpuReadbackError> {
        self.render_internal(
            frame,
            identity,
            &RenderSetup {
                drc_method: None,
                drc_exposure_policy: DrcExposurePolicy::Baseline,
                drc_metered_target_ev100: None,
                drc_profile_adjustment_ev: 0.0,
                wbc_highlight_recovery: features.wbc_highlight_recovery,
                drc_details_amplify: features.drc_details_amplify,
            },
        )
    }

    fn render_internal(
        &self,
        frame: &DecodedRawFrame,
        identity: super::NativeFrameIdentity,
        setup: &RenderSetup<'_>,
    ) -> Result<super::PreviewSurface, WgpuReadbackError> {
        Self::validate_input(&frame.layout, frame.samples().len())?;
        let width = frame.layout.width;
        let height = frame.layout.height;
        let cfa_pattern =
            Self::cfa_pattern(frame.layout.cfa).ok_or(DngReaderError::UnsupportedCfa)?;
        let black_level = frame.metadata.black_levels.first().copied().unwrap_or(0.0) as f32;
        let white_level = frame
            .metadata
            .white_levels
            .first()
            .copied()
            .unwrap_or(4095.0) as f32;
        let drc_local_statistics = build_bayer_local_statistics(
            frame.samples(),
            width,
            height,
            frame.layout.row_stride_samples,
            black_level,
            white_level,
        )
        .map_err(|error| WgpuReadbackError::Resource(error.to_string()))?;
        let preprocess_context = PreprocessContext {
            identity: FrameIdentity {
                frame_index: identity.frame_index,
                run_revision: identity.run_revision,
                method_revision: identity.method_revision,
            },
            width,
            height,
            black_level,
            white_level,
            cfa_pattern,
            as_shot_neutral: frame.metadata.as_shot_neutral,
            as_shot_white_xy: frame.metadata.as_shot_white_xy,
            color_matrix1: frame.metadata.color_matrix1,
            color_matrix2: frame.metadata.color_matrix2,
            calibration_illuminant1_code: frame.metadata.calibration_illuminant1_code,
            calibration_illuminant2_code: frame.metadata.calibration_illuminant2_code,
            camera_calibration1: frame.metadata.camera_calibration1,
            camera_calibration2: frame.metadata.camera_calibration2,
            camera_calibration_signature: frame.metadata.camera_calibration_signature.clone(),
            profile_calibration_signature: frame.metadata.profile_calibration_signature.clone(),
            profile_hue_sat_map_dims: frame.metadata.profile_hue_sat_map_dims,
            profile_hue_sat_map_data1: frame.metadata.profile_hue_sat_map_data1.clone(),
            profile_hue_sat_map_data2: frame.metadata.profile_hue_sat_map_data2.clone(),
            analog_balance: frame.metadata.analog_balance,
            scene_brightness_ev: frame.metadata.exif_brightness_value,
            exposure_deviation_ev: frame.metadata.exif_exposure_bias_value,
            iso: frame.metadata.exif_iso_speed.map(f64::from),
            analog_gain: None,
            digital_gain: None,
            baseline_exposure_ev: frame.metadata.baseline_exposure,
            exposure_time_seconds: positive_ratio(frame.metadata.exif_exposure_time),
            f_number: positive_ratio(frame.metadata.exif_f_number),
            drc_local_statistics: Some(drc_local_statistics),
            drc_exposure_policy: setup.drc_exposure_policy,
            drc_metered_target_ev100: setup.drc_metered_target_ev100,
            drc_profile_adjustment_ev: setup.drc_profile_adjustment_ev,
            drc_gain_offset_ev: None,
            drc_knee: None,
            drc_amplifier: None,
            drc_modulation_curves: None,
            wbc_highlight_recovery: setup.wbc_highlight_recovery,
            wbc_hr_gain: None,
            drc_details_amplify: setup.drc_details_amplify,
        };
        let plan = super::build_normal_graph_plan()?;
        let order = plan
            .execution_order()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let selected = order
            .iter()
            .filter(|id| **id != "raw_source")
            .map(|id| {
                let operator = rime_isp::operator_by_id(id).ok_or_else(|| {
                    OperatorError::UnregisteredOperator {
                        module_id: (*id).to_owned(),
                    }
                })?;
                let method = if *id == "drc" {
                    setup
                        .drc_method
                        .unwrap_or(operator.definition().default_method)
                } else {
                    operator.definition().default_method
                };
                Ok((*id, method))
            })
            .collect::<Result<Vec<_>, OperatorError>>()?;
        let raw_texture = self.upload_raw(frame);
        let mut current: Option<PooledTexture> = None;
        super::execute_operator_methods(&selected, &preprocess_context, |operator, packet| {
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
            let method =
                operator
                    .method(packet.method())
                    .map_err(|_| OperatorError::Preprocess {
                        module_id: operator.definition().id,
                        reason: "operator method manifest is unavailable",
                    })?;
            let output = self
                .acquire_texture(method.output.domain, method.output.format, width, height)
                .map_err(|_| OperatorError::Preprocess {
                    module_id: operator.definition().id,
                    reason: "texture pool is unavailable",
                })?;
            let dispatch = if operator.definition().id == "drc" {
                self.dispatch_drc(packet, input, &output.texture)
            } else {
                self.dispatch(compiled, packet, input, &output.texture)
            };
            dispatch.map_err(|_| OperatorError::Preprocess {
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

    fn dispatch_drc(
        &self,
        packet: &ModuleParameterPacket,
        input: &wgpu::Texture,
        output: &wgpu::Texture,
    ) -> Result<(), WgpuReadbackError> {
        if packet.bytes().len() < 32 {
            return Err(WgpuReadbackError::Resource(
                "DRC scalar packet is incomplete".to_owned(),
            ));
        }
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("drc-scalars"),
                contents: packet.bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let global_resource = packet.resource("tone_lut_global").ok_or_else(|| {
            WgpuReadbackError::Resource("DRC global tone LUT is missing".to_owned())
        })?;
        let global_lut = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(global_resource.id()),
                contents: global_resource.bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let local_lut = packet.resource("tone_lut_local").map(|resource| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(resource.id()),
                    contents: resource.bytes(),
                    usage: wgpu::BufferUsages::STORAGE,
                })
        });
        let modulation_resource = packet.resource("modulation_luts").ok_or_else(|| {
            WgpuReadbackError::Resource("DRC modulation LUTs are missing".to_owned())
        })?;
        let modulation_luts = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(modulation_resource.id()),
                contents: modulation_resource.bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let level_count = u32::from_ne_bytes(
            packet.bytes()[24..28]
                .try_into()
                .expect("validated DRC level count bytes"),
        )
        .clamp(1, 5) as usize;
        let width = input.width();
        let height = input.height();
        let y0 = self.create_drc_texture(wgpu::TextureFormat::R32Float, width, height);
        self.dispatch_drc_pass(
            &self.drc_pipelines.prefilter,
            &uniform,
            &[(1, input)],
            4,
            &y0,
            &[],
        );
        let mut levels = vec![y0];
        for _ in 1..level_count {
            let previous = levels.last().expect("first DRC pyramid level");
            let next = self.create_drc_texture(
                wgpu::TextureFormat::R32Float,
                (previous.width() / 2).max(1),
                (previous.height() / 2).max(1),
            );
            self.dispatch_drc_pass(
                &self.drc_pipelines.downsample,
                &uniform,
                &[(1, previous)],
                4,
                &next,
                &[],
            );
            levels.push(next);
        }

        let mut base = self.dispatch_guided_base(
            levels.last().expect("DRC pyramid is non-empty"),
            &uniform,
            &modulation_luts,
        );
        for index in (0..levels.len().saturating_sub(1)).rev() {
            let candidate = self.create_drc_texture(
                wgpu::TextureFormat::R32Float,
                levels[index].width(),
                levels[index].height(),
            );
            self.dispatch_drc_pass(
                &self.drc_pipelines.reconstruct,
                &uniform,
                &[(1, &levels[index]), (2, &levels[index + 1]), (3, &base)],
                4,
                &candidate,
                &[],
            );
            base = self.dispatch_guided_base(&candidate, &uniform, &modulation_luts);
        }
        let mut tone_buffers = vec![(6, &global_lut), (8, &modulation_luts)];
        let combine = if packet.method() == "01" {
            let local = local_lut.as_ref().ok_or_else(|| {
                WgpuReadbackError::Resource("DRC01 local tone LUT is missing".to_owned())
            })?;
            tone_buffers.push((7, local));
            &self.drc_pipelines.combine_local
        } else {
            &self.drc_pipelines.combine_global
        };
        self.dispatch_drc_pass(
            combine,
            &uniform,
            &[(1, input), (2, &levels[0]), (3, &base)],
            4,
            output,
            &tone_buffers,
        );
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| WgpuReadbackError::Resource(error.to_string()))?;
        Ok(())
    }
    fn dispatch_guided_base(
        &self,
        input: &wgpu::Texture,
        uniform: &wgpu::Buffer,
        modulation_luts: &wgpu::Buffer,
    ) -> wgpu::Texture {
        let width = input.width();
        let height = input.height();
        let coefficients = self.create_drc_texture(wgpu::TextureFormat::Rgba16Float, width, height);
        self.dispatch_drc_pass(
            &self.drc_pipelines.guided_coefficients,
            uniform,
            &[(1, input)],
            5,
            &coefficients,
            &[],
        );
        let output = self.create_drc_texture(wgpu::TextureFormat::R32Float, width, height);
        self.dispatch_drc_pass(
            &self.drc_pipelines.guided_apply_vertical,
            uniform,
            &[(1, &coefficients), (2, input)],
            4,
            &output,
            &[(8, modulation_luts)],
        );
        output
    }

    fn create_drc_texture(
        &self,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("drc-transient"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        })
    }

    fn dispatch_drc_pass(
        &self,
        pipeline: &wgpu::ComputePipeline,
        uniform: &wgpu::Buffer,
        inputs: &[(u32, &wgpu::Texture)],
        output_binding: u32,
        output: &wgpu::Texture,
        buffers: &[(u32, &wgpu::Buffer)],
    ) {
        let input_views = inputs
            .iter()
            .map(|(binding, texture)| {
                (
                    *binding,
                    texture.create_view(&wgpu::TextureViewDescriptor::default()),
                )
            })
            .collect::<Vec<_>>();
        let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let mut entries = Vec::with_capacity(2 + input_views.len() + buffers.len());
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        });
        entries.extend(
            input_views
                .iter()
                .map(|(binding, view)| wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: wgpu::BindingResource::TextureView(view),
                }),
        );
        entries.push(wgpu::BindGroupEntry {
            binding: output_binding,
            resource: wgpu::BindingResource::TextureView(&output_view),
        });
        entries.extend(
            buffers
                .iter()
                .map(|(binding, buffer)| wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: buffer.as_entire_binding(),
                }),
        );
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("drc-pass"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("drc-pass"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("drc-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(output.width().div_ceil(8), output.height().div_ceil(8), 1);
        }
        self.queue.submit([encoder.finish()]);
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
        let parameter_resources = packet
            .resources()
            .iter()
            .map(|resource| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(resource.id()),
                        contents: resource.bytes(),
                        usage: wgpu::BufferUsages::STORAGE,
                    })
            })
            .collect::<Vec<_>>();
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
        for (index, buffer) in parameter_resources.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: 3 + index as u32,
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
        let batch_rows =
            u32::try_from((MAX_READBACK_BYTES / u64::from(row_bytes)).clamp(1, u64::from(height)))
                .unwrap_or(1);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rime-native-readback"),
            size: u64::from(row_bytes) * u64::from(batch_rows),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let visible = width * 16;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        let mut batch_start = 0_u32;
        while batch_start < height {
            let rows = batch_rows.min(height - batch_start);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("rime-native-preview-readback"),
                });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: output,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: batch_start,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row_bytes),
                        rows_per_image: Some(rows),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height: rows,
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
            let batch_bytes = row_bytes as usize * rows as usize;
            for row in mapped[..batch_bytes].chunks_exact(row_bytes as usize) {
                pixels.extend_from_slice(bytemuck::cast_slice::<u8, f32>(&row[..visible as usize]));
            }
            drop(mapped);
            readback.unmap();
            batch_start += rows;
        }
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

fn positive_ratio(value: Option<(u32, u32)>) -> Option<f64> {
    value.and_then(|(numerator, denominator)| {
        (denominator != 0).then(|| f64::from(numerator) / f64::from(denominator))
    })
}

fn texture_format(format: ResourceFormat) -> wgpu::TextureFormat {
    match format {
        ResourceFormat::R16Uint => wgpu::TextureFormat::R16Uint,
        ResourceFormat::R32Float => wgpu::TextureFormat::R32Float,
        ResourceFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
    }
}
