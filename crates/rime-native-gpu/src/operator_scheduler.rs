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
    pub phase: OperatorPhase,
}

pub fn execute_operator_phases(
    order: &[&str],
    preprocess_context: &PreprocessContext,
    mut compute: impl FnMut(
        &'static dyn rime_isp::Operator,
        &rime_isp::ModuleParameterPacket,
    ) -> Result<(), OperatorError>,
) -> Result<Vec<OperatorPhaseEvent>, OperatorError> {
    let operators = order
        .iter()
        .filter(|id| **id != "raw_source")
        .map(|id| {
            rime_isp::operator_by_id(id).ok_or(OperatorError::UnregisteredOperator {
                module_id: (*id).to_owned(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let packets = operators
        .iter()
        .map(|operator| operator.preprocess(preprocess_context))
        .collect::<Result<Vec<_>, _>>()?;
    let mut events = Vec::with_capacity(operators.len() * 3);
    events.extend(operators.iter().map(|operator| OperatorPhaseEvent {
        module_id: operator.definition().id,
        phase: OperatorPhase::Preprocess,
    }));
    for (operator, packet) in operators.iter().zip(&packets) {
        compute(*operator, packet)?;
        events.push(OperatorPhaseEvent {
            module_id: operator.definition().id,
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
    for operator in &operators {
        operator.postprocess(&mut context)?;
        events.push(OperatorPhaseEvent {
            module_id: operator.definition().id,
            phase: OperatorPhase::Postprocess,
        });
    }
    Ok(events)
}
