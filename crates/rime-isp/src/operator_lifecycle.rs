use crate::{
    FrameIdentity, ModuleParameterPacket, Operator, OperatorError, PostprocessContext,
    PreprocessContext, operator_by_id,
};

/// One operator lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorPhase {
    Preprocess,
    Compute,
    Postprocess,
}

/// An observed operator lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatorPhaseEvent {
    pub module_id: &'static str,
    pub method: &'static str,
    pub phase: OperatorPhase,
}

/// Frozen method selection and parameter packets for one frame.
pub struct PreparedOperatorMethods {
    methods: Vec<(&'static dyn Operator, &'static crate::MethodManifest)>,
    packets: Vec<ModuleParameterPacket>,
    events: Vec<OperatorPhaseEvent>,
}

impl PreparedOperatorMethods {
    #[must_use]
    pub fn packets(&self) -> &[ModuleParameterPacket] {
        &self.packets
    }

    #[must_use]
    pub fn events(&self) -> &[OperatorPhaseEvent] {
        &self.events
    }
}

/// Resolves methods and runs their preprocess hooks in order.
///
/// WBC's frozen highlight-recovery gain is threaded into downstream preprocess
/// context exactly once, before DRC preprocessing.
///
/// # Errors
///
/// Returns an operator lookup, method, or preprocess error.
pub fn prepare_operator_methods(
    selected: &[(&str, &str)],
    preprocess_context: &PreprocessContext,
) -> Result<PreparedOperatorMethods, OperatorError> {
    let methods = selected
        .iter()
        .map(|(id, method)| {
            let operator =
                operator_by_id(id).ok_or_else(|| OperatorError::UnregisteredOperator {
                    module_id: (*id).to_owned(),
                })?;
            Ok((operator, operator.method(method)?))
        })
        .collect::<Result<Vec<_>, OperatorError>>()?;

    let mut packets = Vec::with_capacity(methods.len());
    let mut threaded_context = preprocess_context.clone();
    for (operator, method) in &methods {
        let packet = operator.preprocess(method.method, &threaded_context)?;
        if operator.definition().id == "wbc" {
            threaded_context.wbc_hr_gain = Some(
                crate::vfe::white_balance::hr_gain_from_packet(&packet).map_err(|error| {
                    OperatorError::Preprocess {
                        module_id: "wbc",
                        reason: error.reason(),
                    }
                })?,
            );
        }
        packets.push(packet);
    }
    let events = methods
        .iter()
        .map(|(operator, method)| OperatorPhaseEvent {
            module_id: operator.definition().id,
            method: method.method,
            phase: OperatorPhase::Preprocess,
        })
        .collect();
    Ok(PreparedOperatorMethods {
        methods,
        packets,
        events,
    })
}

/// Runs postprocess hooks after all prepared methods have completed compute.
///
/// # Errors
///
/// Returns the first postprocess error.
pub fn complete_operator_methods(
    prepared: &PreparedOperatorMethods,
) -> Result<Vec<OperatorPhaseEvent>, OperatorError> {
    let identity = prepared.packets.first().map_or(
        FrameIdentity {
            frame_index: 0,
            run_revision: 0,
            method_revision: 0,
        },
        ModuleParameterPacket::identity,
    );
    let mut context = PostprocessContext { identity };
    prepared
        .methods
        .iter()
        .map(|(operator, method)| {
            operator.postprocess(method.method, &mut context)?;
            Ok(OperatorPhaseEvent {
                module_id: operator.definition().id,
                method: method.method,
                phase: OperatorPhase::Postprocess,
            })
        })
        .collect()
}

/// Runs ordered preprocess, compute, and postprocess hooks for selected methods.
///
/// WBC's frozen highlight-recovery gain is threaded into downstream preprocess
/// context exactly once, before DRC preprocessing.
///
/// # Errors
///
/// Returns an operator lookup, method, preprocess, compute, or postprocess error.
pub fn execute_operator_methods(
    selected: &[(&str, &str)],
    preprocess_context: &PreprocessContext,
    mut compute: impl FnMut(&'static dyn Operator, &ModuleParameterPacket) -> Result<(), OperatorError>,
) -> Result<Vec<OperatorPhaseEvent>, OperatorError> {
    let prepared = prepare_operator_methods(selected, preprocess_context)?;
    let mut events = prepared.events.clone();
    for ((operator, method), packet) in prepared.methods.iter().zip(&prepared.packets) {
        compute(*operator, packet)?;
        events.push(OperatorPhaseEvent {
            module_id: operator.definition().id,
            method: method.method,
            phase: OperatorPhase::Compute,
        });
    }
    events.extend(complete_operator_methods(&prepared)?);
    Ok(events)
}

/// Runs the default method of each ordered operator.
///
/// # Errors
///
/// Returns an operator lookup, method, preprocess, compute, or postprocess error.
pub fn execute_operator_phases(
    order: &[&str],
    preprocess_context: &PreprocessContext,
    compute: impl FnMut(&'static dyn Operator, &ModuleParameterPacket) -> Result<(), OperatorError>,
) -> Result<Vec<OperatorPhaseEvent>, OperatorError> {
    let selected = order
        .iter()
        .filter(|id| **id != "raw_source")
        .map(|id| {
            let operator =
                operator_by_id(id).ok_or_else(|| OperatorError::UnregisteredOperator {
                    module_id: (*id).to_owned(),
                })?;
            Ok((*id, operator.definition().default_method))
        })
        .collect::<Result<Vec<_>, OperatorError>>()?;
    execute_operator_methods(&selected, preprocess_context, compute)
}
