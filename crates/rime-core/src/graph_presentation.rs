use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeExecutionMode {
    Enabled,
    Bypass,
    Disabled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphTreeKind {
    Group,
    Operator,
    Branch,
    Endpoint,
}


#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GraphIqOverride {
    pub id: String,
    pub module_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IqParameterSource {
    ModuleDefault { module_id: String },
    GraphOverride { module_id: String, override_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IqResolutionError {
    NodeNotFound { node_id: String },
    OverrideNotFound { node_id: String, override_id: String },
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GraphTreeNode {
    pub id: String,
    pub label: String,
    pub parent_id: Option<String>,
    pub kind: GraphTreeKind,
    pub mode: NodeExecutionMode,
    pub execution_node_id: Option<String>,
    pub module_id: Option<String>,
    pub iq_override_id: Option<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub reason: Option<String>,
    pub default_expanded: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GraphPresentationEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub from_port: String,
    pub to_port: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GraphPresentation {
    pub graph_id: String,
    pub root_id: String,
    pub nodes: Vec<GraphTreeNode>,
    pub iq_overrides: Vec<GraphIqOverride>,
    pub edges: Vec<GraphPresentationEdge>,
}

impl GraphPresentation {
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&GraphTreeNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Resolve the graph override or module default for a node.
    ///
    /// # Errors
    ///
    /// Returns an error when the node or its referenced graph override is missing.
    pub fn resolve_iq_source(
        &self,
        node_id: &str,
    ) -> Result<Option<IqParameterSource>, IqResolutionError> {
        let node = self
            .node(node_id)
            .ok_or_else(|| IqResolutionError::NodeNotFound {
                node_id: node_id.into(),
            })?;
        let Some(module_id) = node.module_id.as_deref() else {
            return Ok(None);
        };
        let Some(override_id) = node.iq_override_id.as_deref() else {
            return Ok(Some(IqParameterSource::ModuleDefault {
                module_id: module_id.into(),
            }));
        };
        self.iq_overrides
            .iter()
            .find(|record| record.id == override_id && record.module_id == module_id)
            .map(|record| IqParameterSource::GraphOverride {
                module_id: record.module_id.clone(),
                override_id: record.id.clone(),
            })
            .map(Some)
            .ok_or_else(|| IqResolutionError::OverrideNotFound {
                node_id: node_id.into(),
                override_id: override_id.into(),
            })
    }
}

#[must_use]
pub fn build_top_graph_presentation() -> GraphPresentation {
    let mut nodes = vec![group(
        "isp_pipeline",
        "isp pipeline",
        None,
        NodeExecutionMode::Enabled,
        true,
    )];
    nodes.extend(vfe_nodes());
    nodes.extend(vbe_nodes());
    nodes.extend(vpe_nodes());
    nodes.push(group(
        "encoder",
        "encoder",
        Some("isp_pipeline"),
        NodeExecutionMode::Disabled,
        false,
    ));
    GraphPresentation {
        graph_id: "top".into(),
        root_id: "isp_pipeline".into(),
        nodes,
        iq_overrides: Vec::new(),
        edges: Vec::new(),
    }
}

fn vfe_nodes() -> Vec<GraphTreeNode> {
    vec![
        group(
            "video_front_end",
            "video front end",
            Some("isp_pipeline"),
            NodeExecutionMode::Enabled,
            true,
        ),
        group(
            "sensor_correction",
            "sensor correction",
            Some("video_front_end"),
            NodeExecutionMode::Enabled,
            true,
        ),
        operator(
            "raw_source",
            "raw source",
            "sensor_correction",
            NodeExecutionMode::Enabled,
            Some("raw_source"),
            None,
        ),
        operator(
            "blc",
            "BLC",
            "sensor_correction",
            NodeExecutionMode::Enabled,
            Some("blc"),
            None,
        ),
        operator(
            "sbpc_horizontal",
            "sbpc horizontal",
            "sensor_correction",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
        operator(
            "dbpc",
            "dbpc",
            "sensor_correction",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
        operator(
            "sbpc",
            "static bad pixel correction",
            "sensor_correction",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
        operator(
            "tintless",
            "color shading correction",
            "sensor_correction",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
        operator(
            "lsc",
            "luma shading correction",
            "sensor_correction",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
    ]
}

fn vbe_nodes() -> Vec<GraphTreeNode> {
    let mut nodes = vec![
        group(
            "video_back_end",
            "video back end",
            Some("isp_pipeline"),
            NodeExecutionMode::Enabled,
            true,
        ),
        group(
            "raw_processing",
            "raw processing",
            Some("video_back_end"),
            NodeExecutionMode::Enabled,
            true,
        ),
    ];
    nodes.extend(vbe_raw_nodes());
    nodes.extend(vbe_color_nodes());
    nodes
}

fn vbe_raw_nodes() -> Vec<GraphTreeNode> {
    vec![
        operator(
            "hr",
            "highlight recovery",
            "raw_processing",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
        operator(
            "dynamic_range_compression",
            "dynamic range compression",
            "raw_processing",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
        operator(
            "cac",
            "chromatic aberration correction",
            "raw_processing",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
        operator(
            "raw_noise_reduction",
            "raw noise reduction",
            "raw_processing",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible bayer identity"),
        ),
    ]
}

fn vbe_color_nodes() -> Vec<GraphTreeNode> {
    vec![
        operator(
            "wbc",
            "white balance",
            "video_back_end",
            NodeExecutionMode::Enabled,
            Some("wbc"),
            None,
        ),
        operator(
            "dem",
            "demosaic",
            "video_back_end",
            NodeExecutionMode::Enabled,
            Some("dem"),
            None,
        ),
        operator(
            "pfr",
            "purple-fringe removal",
            "video_back_end",
            NodeExecutionMode::Bypass,
            Some("pfr"),
            Some("not implemented; compatible linear-rgb identity"),
        ),
        operator(
            "color_correction",
            "color correction",
            "video_back_end",
            NodeExecutionMode::Enabled,
            Some("color_correction"),
            None,
        ),
        operator(
            "gamma",
            "gamma",
            "video_back_end",
            NodeExecutionMode::Enabled,
            Some("gamma"),
            None,
        ),
        operator(
            "three_d_lut",
            "3d lut",
            "video_back_end",
            NodeExecutionMode::Bypass,
            None,
            Some("not implemented; compatible encoded rgb identity"),
        ),
        operator(
            "rgb2yuv",
            "rgb to yuv",
            "video_back_end",
            NodeExecutionMode::Enabled,
            Some("rgb2yuv"),
            None,
        ),
        branch(
            "yuv_pyramid",
            "yuv pyramid",
            "video_back_end",
            "extent-changing outputs unavailable",
        ),
    ]
}

fn vpe_nodes() -> Vec<GraphTreeNode> {
    vec![
        group(
            "video_post",
            "video post",
            Some("isp_pipeline"),
            NodeExecutionMode::Disabled,
            false,
        ),
        branch(
            "vpe_sixteenth",
            "1/16 reconstruction",
            "video_post",
            "pyramid input unavailable",
        ),
        branch(
            "vpe_quarter",
            "1/4 reconstruction",
            "video_post",
            "pyramid input unavailable",
        ),
        branch(
            "vpe_full",
            "full reconstruction",
            "video_post",
            "pyramid input unavailable",
        ),
    ]
}

fn group(
    id: &str,
    label: &str,
    parent: Option<&str>,
    mode: NodeExecutionMode,
    expanded: bool,
) -> GraphTreeNode {
    GraphTreeNode {
        id: id.into(),
        label: label.into(),
        parent_id: parent.map(Into::into),
        kind: GraphTreeKind::Group,
        mode,
        execution_node_id: None,
        module_id: None,
        iq_override_id: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        reason: None,
        default_expanded: expanded,
    }
}

fn operator(
    id: &str,
    label: &str,
    parent: &str,
    mode: NodeExecutionMode,
    execution_node_id: Option<&str>,
    reason: Option<&str>,
) -> GraphTreeNode {
    GraphTreeNode {
        id: id.into(),
        label: label.into(),
        parent_id: Some(parent.into()),
        kind: GraphTreeKind::Operator,
        mode,
        execution_node_id: execution_node_id.map(Into::into),
        module_id: None,
        iq_override_id: None,
        inputs: vec!["in".into()],
        outputs: vec!["out".into()],
        reason: reason.map(Into::into),
        default_expanded: false,
    }
}

fn branch(id: &str, label: &str, parent: &str, reason: &str) -> GraphTreeNode {
    GraphTreeNode {
        id: id.into(),
        label: label.into(),
        parent_id: Some(parent.into()),
        kind: GraphTreeKind::Branch,
        mode: NodeExecutionMode::Disabled,
        execution_node_id: None,
        module_id: None,
        iq_override_id: None,
        inputs: vec!["in".into()],
        outputs: Vec::new(),
        reason: Some(reason.into()),
        default_expanded: false,
    }
}
