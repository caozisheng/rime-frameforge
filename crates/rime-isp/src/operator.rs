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
    pub input: OperatorPort,
    pub output: OperatorPort,
    pub output_rime_q_profile: Option<&'static str>,
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
    shader: ShaderAsset,
    preprocess: PreprocessFn,
    postprocess: PostprocessFn,
) -> MethodManifest {
    MethodManifest {
        method,
        shader_entry,
        input,
        output,
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
pub struct ShaderAsset {
    pub method: &'static str,
    pub source: &'static str,
    pub entry_point: &'static str,
    pub bindings: ShaderBindings,
    pub workgroup_size: [u32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameIdentity {
    pub frame_index: u64,
    pub run_revision: u64,
    pub method_revision: u64,
}

#[derive(Clone, Copy, Debug)]
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
}

#[derive(Debug)]
pub struct PostprocessContext {
    pub identity: FrameIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleParameterPacket {
    module_id: &'static str,
    method: &'static str,
    identity: FrameIdentity,
    bytes: [u8; MAX_UNIFORM_BYTES],
    len: usize,
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
    }
}
