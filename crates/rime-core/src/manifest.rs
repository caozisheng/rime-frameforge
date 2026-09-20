use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CompiledGraph, Diagnostic, DiagnosticCode};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalDomain {
    RawBayerSensor,
    RawBayerRimeQ,
    LinearRgb,
    EncodedRgb,
    Yuv,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceFormat {
    R16Uint,
    R32Float,
    Rgba32Float,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewPresentation {
    RawGray,
    Rgb,
    Yuv,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Extent2d {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PortSpec {
    pub id: String,
    pub domain: SignalDomain,
    pub format: ResourceFormat,
    pub extent: Extent2d,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarType {
    F32,
    U32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatisticsPlaneSpec {
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub scalar: ScalarType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatisticsSchema {
    Lcst {
        average_rggb: StatisticsPlaneSpec,
        luma_histogram: StatisticsPlaneSpec,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatisticsPortSpec {
    pub id: String,
    pub schema: StatisticsSchema,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MethodSpec {
    pub method: String,
    pub shader_entry: String,
    pub parameters: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NodeSpec {
    pub id: String,
    pub display_name: String,
    pub shader_entry: Option<String>,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statistics_inputs: Vec<StatisticsPortSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statistics_outputs: Vec<StatisticsPortSpec>,
    pub default_method: String,
    pub methods: Vec<MethodSpec>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PortRef {
    pub node_id: String,
    pub port_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TemporalEdge {
    pub id: String,
    pub from: PortRef,
    pub to: PortRef,
    pub frame_delay: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreviewPortSpec {
    pub node_id: String,
    pub port_id: String,
    pub domain: SignalDomain,
    pub format: ResourceFormat,
    pub extent: Extent2d,
    pub range: String,
    pub channel_layout: String,
    pub presentation: PreviewPresentation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PipelineManifest {
    pub schema_version: u32,
    pub graph_id: String,
    pub graph_kind: String,
    pub manifest_hash: String,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<TemporalEdge>,
    pub preview_outputs: Vec<PreviewPortSpec>,
}

impl PipelineManifest {
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&NodeSpec> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Returns the deterministic execution order of the zero-delay graph.
    ///
    /// # Errors
    ///
    /// Returns a structured manifest diagnostic for unknown nodes or cycles.
    pub fn topological_order(&self) -> Result<Vec<&str>, Diagnostic> {
        let compiled = CompiledGraph::new(self)?;
        let roots: Vec<&str> = self.nodes.iter().map(|node| node.id.as_str()).collect();
        let order = compiled.execution_order_for_outputs(&roots)?;
        order
            .iter()
            .map(|id| {
                self.node(id).map(|node| node.id.as_str()).ok_or_else(|| {
                    Diagnostic::new(
                        DiagnosticCode::ManifestInvalid,
                        format!("compiled graph returned unknown node {id}"),
                    )
                })
            })
            .collect()
    }

    /// Validates IDs, ports, topology, preview outputs, and the canonical hash.
    ///
    /// # Errors
    ///
    /// Returns the first structured contract diagnostic that prevents execution.
    pub fn validate(&self) -> Result<(), Diagnostic> {
        self.validate_unique_ids()?;
        self.validate_node_contracts()?;
        self.validate_edges()?;
        self.topological_order()?;
        self.validate_connected()?;
        self.validate_preview_outputs()?;
        if self.manifest_hash != self.compute_hash() {
            return Err(Diagnostic::new(
                DiagnosticCode::ManifestHashMismatch,
                "manifest hash does not match graph contents",
            ));
        }
        Ok(())
    }

    /// Recomputes the canonical hash after programmatic manifest construction.
    pub fn refresh_hash(&mut self) {
        self.manifest_hash = self.compute_hash();
    }

    fn validate_unique_ids(&self) -> Result<(), Diagnostic> {
        let mut node_ids = HashSet::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if !node_ids.insert(node.id.as_str()) {
                return Err(Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("duplicate node id {}", node.id),
                ));
            }
        }
        let mut edge_ids = HashSet::with_capacity(self.edges.len());
        for edge in &self.edges {
            if !edge_ids.insert(edge.id.as_str()) {
                return Err(Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("duplicate edge id {}", edge.id),
                ));
            }
        }
        Ok(())
    }

    fn validate_node_contracts(&self) -> Result<(), Diagnostic> {
        for node in &self.nodes {
            let mut input_ids = HashSet::with_capacity(
                node.inputs
                    .len()
                    .saturating_add(node.statistics_inputs.len()),
            );
            for id in node
                .inputs
                .iter()
                .map(|port| port.id.as_str())
                .chain(node.statistics_inputs.iter().map(|port| port.id.as_str()))
            {
                if !input_ids.insert(id) {
                    return Err(Diagnostic::new(
                        DiagnosticCode::ManifestInvalid,
                        format!("duplicate input port {}.{id}", node.id),
                    ));
                }
            }
            let mut output_ids = HashSet::with_capacity(
                node.outputs
                    .len()
                    .saturating_add(node.statistics_outputs.len()),
            );
            for id in node
                .outputs
                .iter()
                .map(|port| port.id.as_str())
                .chain(node.statistics_outputs.iter().map(|port| port.id.as_str()))
            {
                if !output_ids.insert(id) {
                    return Err(Diagnostic::new(
                        DiagnosticCode::ManifestInvalid,
                        format!("duplicate output port {}.{id}", node.id),
                    ));
                }
            }
            if !node.inputs.is_empty() && node.shader_entry.is_none() {
                return Err(Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("processing node {} has no shader entry", node.id),
                ));
            }
        }
        Ok(())
    }

    fn validate_connected(&self) -> Result<(), Diagnostic> {
        if self.nodes.len() <= 1 {
            return Ok(());
        }
        let connected: HashSet<&str> = self
            .edges
            .iter()
            .flat_map(|edge| [edge.from.node_id.as_str(), edge.to.node_id.as_str()])
            .collect();
        if let Some(node) = self
            .nodes
            .iter()
            .find(|node| !connected.contains(node.id.as_str()))
        {
            return Err(Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("node {} is isolated", node.id),
            ));
        }
        Ok(())
    }

    fn validate_edges(&self) -> Result<(), Diagnostic> {
        for edge in &self.edges {
            let output = self.port_contract(&edge.from, false)?;
            let input = self.port_contract(&edge.to, true)?;
            let compatible = match (output, input) {
                (PortContract::Image(output), PortContract::Image(input)) => {
                    output.domain == input.domain
                        && output.format == input.format
                        && output.extent == input.extent
                }
                (PortContract::Statistics(output), PortContract::Statistics(input)) => {
                    output.schema == input.schema
                }
                (PortContract::Image(_), PortContract::Statistics(_))
                | (PortContract::Statistics(_), PortContract::Image(_)) => false,
            };
            if !compatible {
                return Err(Diagnostic::new(
                    DiagnosticCode::PortContractMismatch,
                    format!("edge {} connects incompatible ports", edge.id),
                ));
            }
        }
        Ok(())
    }

    fn validate_preview_outputs(&self) -> Result<(), Diagnostic> {
        for preview in &self.preview_outputs {
            let reference = PortRef {
                node_id: preview.node_id.clone(),
                port_id: preview.port_id.clone(),
            };
            let port = self.port(&reference, false)?;
            if port.domain != preview.domain
                || port.format != preview.format
                || port.extent != preview.extent
            {
                return Err(Diagnostic::new(
                    DiagnosticCode::PreviewUnavailable,
                    format!(
                        "preview {}.{} mismatches its output",
                        preview.node_id, preview.port_id
                    ),
                ));
            }
        }
        Ok(())
    }

    fn port(&self, reference: &PortRef, input: bool) -> Result<&PortSpec, Diagnostic> {
        let node = self.node(&reference.node_id).ok_or_else(|| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("unknown node {}", reference.node_id),
            )
        })?;
        let ports = if input { &node.inputs } else { &node.outputs };
        ports
            .iter()
            .find(|port| port.id == reference.port_id)
            .ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("unknown port {}.{}", reference.node_id, reference.port_id),
                )
            })
    }

    fn port_contract(
        &self,
        reference: &PortRef,
        input: bool,
    ) -> Result<PortContract<'_>, Diagnostic> {
        let node = self.node(&reference.node_id).ok_or_else(|| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("unknown node {}", reference.node_id),
            )
        })?;
        let image_ports = if input { &node.inputs } else { &node.outputs };
        if let Some(port) = image_ports.iter().find(|port| port.id == reference.port_id) {
            return Ok(PortContract::Image(port));
        }
        let statistics_ports = if input {
            &node.statistics_inputs
        } else {
            &node.statistics_outputs
        };
        statistics_ports
            .iter()
            .find(|port| port.id == reference.port_id)
            .map(PortContract::Statistics)
            .ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("unknown port {}.{}", reference.node_id, reference.port_id),
                )
            })
    }

    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.schema_version.to_le_bytes());
        update_string(&mut hasher, &self.graph_id);
        update_string(&mut hasher, &self.graph_kind);
        for node in &self.nodes {
            update_string(&mut hasher, &node.id);
            update_string(&mut hasher, &node.display_name);
            update_optional_string(&mut hasher, node.shader_entry.as_deref());
            update_string(&mut hasher, &node.default_method);
            update_ports(&mut hasher, &node.inputs);
            update_statistics_ports(&mut hasher, &node.statistics_inputs);
            for method in &node.methods {
                update_string(&mut hasher, &method.method);
                update_string(&mut hasher, &method.shader_entry);
                for parameter in &method.parameters {
                    update_string(&mut hasher, parameter);
                }
            }
            update_ports(&mut hasher, &node.outputs);
            update_statistics_ports(&mut hasher, &node.statistics_outputs);
        }
        for edge in &self.edges {
            update_string(&mut hasher, &edge.id);
            update_string(&mut hasher, &edge.from.node_id);
            update_string(&mut hasher, &edge.from.port_id);
            update_string(&mut hasher, &edge.to.node_id);
            update_string(&mut hasher, &edge.to.port_id);
            hasher.update(edge.frame_delay.to_le_bytes());
        }
        for preview in &self.preview_outputs {
            update_string(&mut hasher, &preview.node_id);
            update_string(&mut hasher, &preview.port_id);
            update_port_contract(&mut hasher, preview.domain, preview.format, &preview.extent);
            update_string(&mut hasher, &preview.range);
            update_string(&mut hasher, &preview.channel_layout);
            hasher.update([preview.presentation as u8]);
        }
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortContract<'a> {
    Image(&'a PortSpec),
    Statistics(&'a StatisticsPortSpec),
}

fn update_ports(hasher: &mut Sha256, ports: &[PortSpec]) {
    for port in ports {
        update_string(hasher, &port.id);
        update_port_contract(hasher, port.domain, port.format, &port.extent);
    }
}

fn update_statistics_ports(hasher: &mut Sha256, ports: &[StatisticsPortSpec]) {
    for port in ports {
        update_string(hasher, &port.id);
        match &port.schema {
            StatisticsSchema::Lcst {
                average_rggb,
                luma_histogram,
            } => {
                hasher.update([0]);
                update_statistics_plane(hasher, average_rggb);
                update_statistics_plane(hasher, luma_histogram);
            }
        }
    }
}

fn update_statistics_plane(hasher: &mut Sha256, plane: &StatisticsPlaneSpec) {
    hasher.update(plane.width.to_le_bytes());
    hasher.update(plane.height.to_le_bytes());
    hasher.update(plane.channels.to_le_bytes());
    hasher.update([plane.scalar as u8]);
}

fn update_port_contract(
    hasher: &mut Sha256,
    domain: SignalDomain,
    format: ResourceFormat,
    extent: &Extent2d,
) {
    hasher.update([domain as u8, format as u8]);
    hasher.update(extent.width.to_le_bytes());
    hasher.update(extent.height.to_le_bytes());
}

fn update_string(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_le_bytes());
    hasher.update(value.as_bytes());
}

fn update_optional_string(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            update_string(hasher, value);
        }
        None => hasher.update([0]),
    }
}
