use rime_isp::{FrameIdentity, OperatorError, PostprocessContext, PreprocessContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorPhase {
    Preprocess,
    Compute,
    Postprocess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatorPhaseEvent {
    pub module_id: &'static str,
    pub method: &'static str,
    pub phase: OperatorPhase,
}

pub fn execute_operator_phases(
    order: &[&str],
    preprocess_context: &PreprocessContext,
    compute: impl FnMut(
        &'static dyn rime_isp::Operator,
        &rime_isp::ModuleParameterPacket,
    ) -> Result<(), OperatorError>,
) -> Result<Vec<OperatorPhaseEvent>, OperatorError> {
    let selected = order
        .iter()
        .filter(|id| **id != "raw_source")
        .map(|id| {
            let operator = rime_isp::operator_by_id(id).ok_or_else(|| {
                OperatorError::UnregisteredOperator {
                    module_id: (*id).to_owned(),
                }
            })?;
            Ok((*id, operator.definition().default_method))
        })
        .collect::<Result<Vec<_>, OperatorError>>()?;
    execute_operator_methods(&selected, preprocess_context, compute)
}

pub fn execute_operator_methods(
    selected: &[(&str, &str)],
    preprocess_context: &PreprocessContext,
    mut compute: impl FnMut(
        &'static dyn rime_isp::Operator,
        &rime_isp::ModuleParameterPacket,
    ) -> Result<(), OperatorError>,
) -> Result<Vec<OperatorPhaseEvent>, OperatorError> {
    let methods = selected
        .iter()
        .map(|(id, method)| {
            let operator = rime_isp::operator_by_id(id).ok_or_else(|| {
                OperatorError::UnregisteredOperator {
                    module_id: (*id).to_owned(),
                }
            })?;
            let manifest = operator.method(method)?;
            Ok((operator, manifest))
        })
        .collect::<Result<Vec<_>, OperatorError>>()?;
    let packets = methods
        .iter()
        .map(|(operator, method)| operator.preprocess(method.method, preprocess_context))
        .collect::<Result<Vec<_>, _>>()?;
    let mut events = Vec::with_capacity(methods.len() * 3);
    events.extend(methods.iter().map(|(operator, method)| OperatorPhaseEvent {
        module_id: operator.definition().id,
        method: method.method,
        phase: OperatorPhase::Preprocess,
    }));
    for ((operator, method), packet) in methods.iter().zip(&packets) {
        compute(*operator, packet)?;
        events.push(OperatorPhaseEvent {
            module_id: operator.definition().id,
            method: method.method,
            phase: OperatorPhase::Compute,
        });
    }
    let mut context = PostprocessContext {
        identity: FrameIdentity {
            frame_index: preprocess_context.identity.frame_index,
            run_revision: preprocess_context.identity.run_revision,
            method_revision: preprocess_context.identity.method_revision,
        },
    };
    for (operator, method) in &methods {
        operator.postprocess(method.method, &mut context)?;
        events.push(OperatorPhaseEvent {
            module_id: operator.definition().id,
            method: method.method,
            phase: OperatorPhase::Postprocess,
        });
    }
    Ok(events)
}
