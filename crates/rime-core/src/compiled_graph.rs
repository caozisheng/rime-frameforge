use std::collections::{HashMap, HashSet};

use petgraph::{
    Direction,
    algo::toposort,
    graph::{DiGraph, NodeIndex},
};

use crate::{Diagnostic, DiagnosticCode, PipelineManifest};

pub struct CompiledGraph {
    graph: DiGraph<String, usize>,
    node_by_id: HashMap<String, NodeIndex>,
}

impl CompiledGraph {
    /// Builds the zero-delay computation graph from a validated manifest shape.
    ///
    /// # Errors
    ///
    /// Returns a structured diagnostic for unknown edge endpoints or cycles.
    pub fn new(manifest: &PipelineManifest) -> Result<Self, Diagnostic> {
        let mut graph = DiGraph::new();
        let mut node_by_id = HashMap::with_capacity(manifest.nodes.len());
        for node in &manifest.nodes {
            let index = graph.add_node(node.id.clone());
            node_by_id.insert(node.id.clone(), index);
        }
        for (edge_index, edge) in manifest
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.frame_delay == 0)
        {
            let from = node_by_id.get(&edge.from.node_id).copied().ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("edge {} starts at an unknown node", edge.id),
                )
            })?;
            let to = node_by_id.get(&edge.to.node_id).copied().ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("edge {} targets an unknown node", edge.id),
                )
            })?;
            graph.add_edge(from, to, edge_index);
        }
        toposort(&graph, None).map_err(|cycle| {
            Diagnostic::new(
                DiagnosticCode::ManifestTopologyCycle,
                format!(
                    "zero-delay graph cycle contains node {}",
                    graph[cycle.node_id()]
                ),
            )
        })?;
        Ok(Self { graph, node_by_id })
    }

    /// Resolves output roots into a stable forward execution order.
    ///
    /// # Errors
    ///
    /// Returns `ManifestInvalid` when an output root is unknown.
    pub fn execution_order_for_outputs(
        &self,
        output_node_ids: &[&str],
    ) -> Result<Vec<String>, Diagnostic> {
        let mut reachable = HashSet::new();
        let mut stack = Vec::with_capacity(output_node_ids.len());
        for output in output_node_ids {
            let index = self.node_by_id.get(*output).copied().ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("unknown output root {output}"),
                )
            })?;
            stack.push(index);
        }
        while let Some(index) = stack.pop() {
            if !reachable.insert(index) {
                continue;
            }
            stack.extend(self.graph.neighbors_directed(index, Direction::Incoming));
        }
        let order = toposort(&self.graph, None).map_err(|cycle| {
            Diagnostic::new(
                DiagnosticCode::ManifestTopologyCycle,
                format!(
                    "zero-delay graph cycle contains node {}",
                    self.graph[cycle.node_id()]
                ),
            )
        })?;
        Ok(order
            .into_iter()
            .filter(|index| reachable.contains(index))
            .map(|index| self.graph[index].clone())
            .collect())
    }
}
