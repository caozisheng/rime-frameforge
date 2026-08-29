use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatorPort {
    pub domain: SignalDomain,
    pub format: ResourceFormat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatorMethod {
    pub method: &'static str,
    pub shader_entry: &'static str,
    pub input: OperatorPort,
    pub output: OperatorPort,
    pub parameters: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatorDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub mode: NodeExecutionMode,
    pub input: OperatorPort,
    pub output: OperatorPort,
    pub output_rime_q_profile: Option<&'static str>,
    pub default_method: &'static str,
    pub methods: &'static [OperatorMethod],
}

pub const fn method(
    method: &'static str,
    shader_entry: &'static str,
    input: OperatorPort,
    output: OperatorPort,
    parameters: &'static str,
) -> OperatorMethod {
    OperatorMethod {
        method,
        shader_entry,
        input,
        output,
        parameters,
    }
}
