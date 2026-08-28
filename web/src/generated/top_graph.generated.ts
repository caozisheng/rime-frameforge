export const topGraphPresentation = {
  "graph_id": "top",
  "root_id": "isp_pipeline",
  "nodes": [
    {
      "id": "isp_pipeline",
      "label": "isp pipeline",
      "parent_id": null,
      "kind": "group",
      "mode": "enabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "video_front_end",
      "label": "video front end",
      "parent_id": "isp_pipeline",
      "kind": "group",
      "mode": "enabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "sensor_correction",
      "label": "sensor correction",
      "parent_id": "video_front_end",
      "kind": "group",
      "mode": "enabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "raw_source",
      "label": "raw source",
      "parent_id": "sensor_correction",
      "kind": "operator",
      "mode": "enabled",
      "execution_node_id": "raw_source",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "blc",
      "label": "BLC",
      "parent_id": "sensor_correction",
      "kind": "operator",
      "mode": "enabled",
      "execution_node_id": "blc",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "sbpc_horizontal",
      "label": "sbpc horizontal",
      "parent_id": "sensor_correction",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "dbpc",
      "label": "dbpc",
      "parent_id": "sensor_correction",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "sbpc",
      "label": "static bad pixel correction",
      "parent_id": "sensor_correction",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "tintless",
      "label": "color shading correction",
      "parent_id": "sensor_correction",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "lsc",
      "label": "luma shading correction",
      "parent_id": "sensor_correction",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "video_back_end",
      "label": "video back end",
      "parent_id": "isp_pipeline",
      "kind": "group",
      "mode": "enabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "raw_processing",
      "label": "raw processing",
      "parent_id": "video_back_end",
      "kind": "group",
      "mode": "enabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "hr",
      "label": "highlight recovery",
      "parent_id": "raw_processing",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "dynamic_range_compression",
      "label": "dynamic range compression",
      "parent_id": "raw_processing",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "cac",
      "label": "chromatic aberration correction",
      "parent_id": "raw_processing",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "raw_noise_reduction",
      "label": "raw noise reduction",
      "parent_id": "raw_processing",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible bayer identity",
      "default_expanded": false
    },
    {
      "id": "wbc",
      "label": "white balance",
      "parent_id": "video_back_end",
      "kind": "operator",
      "mode": "enabled",
      "execution_node_id": "wbc",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "dem",
      "label": "demosaic",
      "parent_id": "video_back_end",
      "kind": "operator",
      "mode": "enabled",
      "execution_node_id": "dem",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "pfr",
      "label": "purple-fringe removal",
      "parent_id": "video_back_end",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "pfr",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible linear-rgb identity",
      "default_expanded": false
    },
    {
      "id": "color_correction",
      "label": "color correction",
      "parent_id": "video_back_end",
      "kind": "operator",
      "mode": "enabled",
      "execution_node_id": "color_correction",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "gamma",
      "label": "gamma",
      "parent_id": "video_back_end",
      "kind": "operator",
      "mode": "enabled",
      "execution_node_id": "gamma",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "three_d_lut",
      "label": "3d lut",
      "parent_id": "video_back_end",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "not implemented; compatible encoded rgb identity",
      "default_expanded": false
    },
    {
      "id": "rgb2yuv",
      "label": "rgb to yuv",
      "parent_id": "video_back_end",
      "kind": "operator",
      "mode": "enabled",
      "execution_node_id": "rgb2yuv",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "yuv_pyramid",
      "label": "yuv pyramid",
      "parent_id": "video_back_end",
      "kind": "branch",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [],
      "reason": "extent-changing outputs unavailable",
      "default_expanded": false
    },
    {
      "id": "video_post",
      "label": "video post",
      "parent_id": "isp_pipeline",
      "kind": "group",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "vpe_sixteenth",
      "label": "1/16 reconstruction",
      "parent_id": "video_post",
      "kind": "branch",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_quarter",
      "label": "1/4 reconstruction",
      "parent_id": "video_post",
      "kind": "branch",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_full",
      "label": "full reconstruction",
      "parent_id": "video_post",
      "kind": "branch",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "encoder",
      "label": "encoder",
      "parent_id": "isp_pipeline",
      "kind": "group",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": false
    }
  ],
  "iq_overrides": [],
  "edges": []
} as const;
