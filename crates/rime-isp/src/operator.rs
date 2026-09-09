use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatorPort {
    pub domain: SignalDomain,
    pub format: ResourceFormat,
}

#[derive(Clone, Copy, Debug)]
pub struct MethodManifest {
    pub method: &'static str,
    pub shader_entry: &'static str,
    pub input: OperatorPort,
    pub output: OperatorPort,
    pub output_rime_q_profile: Option<&'static str>,
    pub parameters: &'static str,
    pub shader: ShaderAsset,
    pub preprocess: PreprocessFn,
    pub postprocess: PostprocessFn,
}

#[derive(Clone, Copy, Debug)]
pub struct OperatorDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub mode: NodeExecutionMode,
    pub default_method: &'static str,
    pub methods: &'static [MethodManifest],
}

#[expect(
    clippy::too_many_arguments,
    reason = "a method manifest explicitly binds metadata, shader, preprocess, and postprocess"
)]
pub const fn method_manifest(
    method: &'static str,
    shader_entry: &'static str,
    input: OperatorPort,
    output: OperatorPort,
    parameters: &'static str,
    output_rime_q_profile: Option<&'static str>,
    shader: ShaderAsset,
    preprocess: PreprocessFn,
    postprocess: PostprocessFn,
) -> MethodManifest {
    MethodManifest {
        method,
        shader_entry,
        input,
        output,
        output_rime_q_profile,
        parameters,
        shader,
        preprocess,
        postprocess,
    }
}

pub const MAX_UNIFORM_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShaderBindings {
    pub input: u32,
    pub output: u32,
    pub uniform: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShaderBindingKind {
    Texture,
    StorageTexture,
    UniformBuffer,
    StorageBuffer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShaderBindingAccess {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShaderStageBinding {
    pub binding: u32,
    pub resource: &'static str,
    pub kind: ShaderBindingKind,
    pub access: ShaderBindingAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShaderStageAsset {
    pub entry_point: &'static str,
    pub bindings: &'static [ShaderStageBinding],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShaderAsset {
    pub method: &'static str,
    pub source: &'static str,
    pub entry_point: &'static str,
    pub bindings: ShaderBindings,
    pub workgroup_size: [u32; 3],
    pub stages: &'static [ShaderStageAsset],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameIdentity {
    pub frame_index: u64,
    pub run_revision: u64,
    pub method_revision: u64,
}

#[derive(Clone, Debug)]
pub struct PreprocessContext {
    pub identity: FrameIdentity,
    pub width: u32,
    pub height: u32,
    pub black_level: f32,
    pub white_level: f32,
    pub cfa_pattern: [u32; 4],
    pub as_shot_neutral: Option<[f64; 3]>,
    pub as_shot_white_xy: Option<[f64; 2]>,
    pub color_matrix1: [f64; 9],
    pub color_matrix2: Option<[f64; 9]>,
    pub calibration_illuminant1_code: Option<u16>,
    pub calibration_illuminant2_code: Option<u16>,
    pub camera_calibration1: Option<[f64; 9]>,
    pub camera_calibration2: Option<[f64; 9]>,
    pub camera_calibration_signature: Option<String>,
    pub profile_calibration_signature: Option<String>,
    pub profile_hue_sat_map_dims: Option<[u32; 3]>,
    pub profile_hue_sat_map_data1: Option<Vec<f32>>,
    pub profile_hue_sat_map_data2: Option<Vec<f32>>,
    pub analog_balance: Option<[f64; 3]>,
    pub scene_brightness_ev: Option<f64>,
    pub exposure_deviation_ev: Option<f64>,
    pub iso: Option<f64>,
    pub analog_gain: Option<f64>,
    pub digital_gain: Option<f64>,
    pub baseline_exposure_ev: Option<f64>,
    pub exposure_time_seconds: Option<f64>,
    pub f_number: Option<f64>,
    pub drc_local_statistics: Option<crate::vbe::drc::DrcLocalStatistics>,
    pub drc_exposure_policy: crate::vbe::drc::DrcExposurePolicy,
    pub drc_metered_target_ev100: Option<f64>,
    pub drc_profile_adjustment_ev: f64,
    /// Optional DRC gain offset in EV; omitted means zero.
    pub drc_gain_offset_ev: Option<f32>,
    /// Optional DRC tone knee; omitted means one.
    pub drc_knee: Option<f32>,
    /// Optional DRC detail amplifier; omitted means the Sony reference default of three.
    pub drc_amplifier: Option<f32>,
    /// WBC internal highlight-recovery switch (wbc00); default off.
    pub wbc_highlight_recovery: bool,
}

#[derive(Debug)]
pub struct PostprocessContext {
    pub identity: FrameIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleParameterResource {
    id: &'static str,
    extent: [u32; 3],
    bytes: Vec<u8>,
}

impl ModuleParameterResource {
    #[must_use]
    pub const fn new(id: &'static str, extent: [u32; 3], bytes: Vec<u8>) -> Self {
        Self { id, extent, bytes }
    }

    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.id
    }

    #[must_use]
    pub const fn extent(&self) -> [u32; 3] {
        self.extent
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleParameterPacket {
    module_id: &'static str,
    method: &'static str,
    identity: FrameIdentity,
    bytes: [u8; MAX_UNIFORM_BYTES],
    len: usize,
    resources: Vec<ModuleParameterResource>,
}

impl ModuleParameterPacket {
    #[must_use]
    pub const fn empty(
        module_id: &'static str,
        method: &'static str,
        identity: FrameIdentity,
    ) -> Self {
        Self {
            module_id,
            method,
            identity,
            bytes: [0; MAX_UNIFORM_BYTES],
            len: 0,
            resources: Vec::new(),
        }
    }

    /// Creates a frozen uniform packet for one operator invocation.
    ///
    /// # Errors
    ///
    /// Returns `UniformTooLarge` when the module parameter block exceeds the
    /// fixed packet capacity.
    pub fn new(
        module_id: &'static str,
        method: &'static str,
        identity: FrameIdentity,
        uniform: &[u8],
    ) -> Result<Self, OperatorError> {
        if uniform.len() > MAX_UNIFORM_BYTES {
            return Err(OperatorError::UniformTooLarge {
                module_id,
                actual: uniform.len(),
                maximum: MAX_UNIFORM_BYTES,
            });
        }
        let mut packet = Self::empty(module_id, method, identity);
        packet.bytes[..uniform.len()].copy_from_slice(uniform);
        packet.len = uniform.len();
        Ok(packet)
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    /// Adds one immutable parameter resource to this invocation.
    ///
    /// # Errors
    ///
    /// Returns `DuplicateResource` when the resource identifier is already present.
    pub fn push_resource(
        &mut self,
        resource: ModuleParameterResource,
    ) -> Result<(), OperatorError> {
        if self
            .resources
            .iter()
            .any(|current| current.id == resource.id)
        {
            return Err(OperatorError::DuplicateResource {
                module_id: self.module_id,
                resource_id: resource.id,
            });
        }
        self.resources.push(resource);
        Ok(())
    }

    #[must_use]
    pub fn resource(&self, id: &str) -> Option<&ModuleParameterResource> {
        self.resources.iter().find(|resource| resource.id == id)
    }

    #[must_use]
    pub fn resources(&self) -> &[ModuleParameterResource] {
        &self.resources
    }

    #[must_use]
    pub const fn module_id(&self) -> &'static str {
        self.module_id
    }

    #[must_use]
    pub const fn method(&self) -> &'static str {
        self.method
    }

    #[must_use]
    pub const fn identity(&self) -> FrameIdentity {
        self.identity
    }
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum OperatorError {
    #[error("graph references unregistered operator `{module_id}`")]
    UnregisteredOperator { module_id: String },
    #[error("operator `{module_id}` has no method `{method}`")]
    UnknownMethod {
        module_id: &'static str,
        method: String,
    },
    #[error("operator `{module_id}` uniform is {actual} bytes; maximum is {maximum}")]
    UniformTooLarge {
        module_id: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("operator `{module_id}` has duplicate parameter resource `{resource_id}`")]
    DuplicateResource {
        module_id: &'static str,
        resource_id: &'static str,
    },
    #[error("operator `{module_id}` preprocessing failed: {reason}")]
    Preprocess {
        module_id: &'static str,
        reason: &'static str,
    },
}

pub type PreprocessFn = fn(
    &PreprocessContext,
    &'static str,
    &'static str,
) -> Result<ModuleParameterPacket, OperatorError>;
pub type PostprocessFn = fn(&mut PostprocessContext) -> Result<(), OperatorError>;

pub trait Operator: Sync {
    fn definition(&self) -> &'static OperatorDefinition;

    /// Resolves one method to its complete immutable manifest.
    ///
    /// # Errors
    ///
    /// Returns `UnknownMethod` when the method is not registered.
    fn method(&self, method: &str) -> Result<&'static MethodManifest, OperatorError> {
        self.definition()
            .methods
            .iter()
            .find(|candidate| candidate.method == method)
            .ok_or_else(|| OperatorError::UnknownMethod {
                module_id: self.definition().id,
                method: method.to_owned(),
            })
    }

    /// Freezes parameters for the selected method.
    ///
    /// # Errors
    ///
    /// Returns the selected method's preprocessing error.
    fn preprocess(
        &self,
        method: &str,
        context: &PreprocessContext,
    ) -> Result<ModuleParameterPacket, OperatorError> {
        let method = self.method(method)?;
        (method.preprocess)(context, self.definition().id, method.method)
    }

    /// Resolves the selected method's shader.
    ///
    /// # Errors
    ///
    /// Returns `UnknownMethod` when the method is not registered.
    fn shader(&self, method: &str) -> Result<&'static ShaderAsset, OperatorError> {
        Ok(&self.method(method)?.shader)
    }

    /// Runs the selected method's result processing.
    ///
    /// # Errors
    ///
    /// Returns the selected method's postprocessing error.
    fn postprocess(
        &self,
        method: &str,
        context: &mut PostprocessContext,
    ) -> Result<(), OperatorError> {
        (self.method(method)?.postprocess)(context)
    }
}

pub struct StaticOperator {
    pub definition: &'static OperatorDefinition,
}

impl Operator for StaticOperator {
    fn definition(&self) -> &'static OperatorDefinition {
        self.definition
    }
}

/// Creates an empty parameter packet for a method without CPU preprocessing.
///
/// # Errors
///
/// This helper currently cannot fail; its `Result` matches `PreprocessFn`.
pub fn empty_preprocess(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    Ok(ModuleParameterPacket::empty(
        module_id,
        method,
        context.identity,
    ))
}

/// Completes a method without CPU result processing.
///
/// # Errors
///
/// This helper currently cannot fail; its `Result` matches `PostprocessFn`.
pub const fn empty_postprocess(_context: &mut PostprocessContext) -> Result<(), OperatorError> {
    Ok(())
}

pub const fn shader(
    method: &'static str,
    source: &'static str,
    entry_point: &'static str,
    bindings: ShaderBindings,
) -> ShaderAsset {
    ShaderAsset {
        method,
        source,
        entry_point,
        bindings,
        workgroup_size: [8, 8, 1],
        stages: &[],
    }
}

#[must_use]
pub const fn shader_plan(
    method: &'static str,
    source: &'static str,
    entry_point: &'static str,
    bindings: ShaderBindings,
    stages: &'static [ShaderStageAsset],
) -> ShaderAsset {
    ShaderAsset {
        method,
        source,
        entry_point,
        bindings,
        workgroup_size: [8, 8, 1],
        stages,
    }
}
