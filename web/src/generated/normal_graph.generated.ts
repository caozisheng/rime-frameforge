export const normalGraphPresentation = {
  "graph_id": "normal",
  "root_id": "normal",
  "nodes": [
    {
      "id": "normal",
      "label": "normal",
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
      "id": "raw_source",
      "label": "RAW Source",
      "parent_id": "normal",
      "kind": "endpoint",
      "mode": "enabled",
      "execution_node_id": "raw_source",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [
        "out"
      ],
      "reason": null,
      "default_expanded": false
    },
    {
      "id": "vfe",
      "label": "VFE",
      "parent_id": "normal",
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
      "id": "blc",
      "label": "BLC",
      "parent_id": "vfe",
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
      "label": "SBPC-H",
      "parent_id": "vfe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "sbpc_horizontal",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "dbpc",
      "label": "DBPC",
      "parent_id": "vfe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "dbpc",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "sbpc",
      "label": "SBPC",
      "parent_id": "vfe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "sbpc",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "Static Bad Pixel Correction; method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "tintless",
      "label": "TINTLESS",
      "parent_id": "vfe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "tintless",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "color shading correction; method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "lsc",
      "label": "LSC",
      "parent_id": "vfe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "lsc",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "luma shading correction; method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "vbe",
      "label": "VBE",
      "parent_id": "normal",
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
      "label": "HR",
      "parent_id": "vbe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "hr",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "drc",
      "label": "DRC",
      "parent_id": "vbe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "drc",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "cac",
      "label": "CAC",
      "parent_id": "vbe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "cac",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "raw_nr",
      "label": "RAW-NR",
      "parent_id": "vbe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "raw_nr",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "wbc",
      "label": "WBC",
      "parent_id": "vbe",
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
      "label": "DEM",
      "parent_id": "vbe",
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
      "label": "PFR",
      "parent_id": "vbe",
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
      "reason": "Purple-Fringe Removal; method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "color_correction",
      "label": "CCM 8 x 3 x 3",
      "parent_id": "vbe",
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
      "label": "Gamma",
      "parent_id": "vbe",
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
      "label": "3D LUT 17³",
      "parent_id": "vbe",
      "kind": "operator",
      "mode": "bypass",
      "execution_node_id": "three_d_lut",
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "method 00: identity bypass",
      "default_expanded": false
    },
    {
      "id": "rgb2yuv",
      "label": "RGB2YUV",
      "parent_id": "vbe",
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
      "id": "pyrd",
      "label": "PYRD",
      "parent_id": "vbe",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "full",
        "quarter",
        "sixteenth"
      ],
      "reason": "Gaussian Pyramid Decomposition; one input, three scale outputs",
      "default_expanded": false
    },
    {
      "id": "vpe",
      "label": "VPE",
      "parent_id": "normal",
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
      "id": "vpe_16_pass",
      "label": "pass-1",
      "parent_id": "vpe",
      "kind": "group",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "vpe_16_pyrc",
      "label": "PYRC",
      "parent_id": "vpe_16_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in",
        "feedback"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_16_mctf_1",
      "label": "MCTF",
      "parent_id": "vpe_16_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": "mctf",
      "iq_override_id": "mctf_1",
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_16_lce",
      "label": "LCE",
      "parent_id": "vpe_16_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_16_ce",
      "label": "CE",
      "parent_id": "vpe_16_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_16_mctf_2",
      "label": "MCTF",
      "parent_id": "vpe_16_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": "mctf",
      "iq_override_id": "mctf_2",
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_16_sharpen",
      "label": "Sharpen",
      "parent_id": "vpe_16_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_4_pass",
      "label": "pass-2",
      "parent_id": "vpe",
      "kind": "group",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "vpe_4_pyrc",
      "label": "PYRC",
      "parent_id": "vpe_4_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in",
        "feedback"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_4_mctf_1",
      "label": "MCTF",
      "parent_id": "vpe_4_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": "mctf",
      "iq_override_id": "mctf_1",
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_4_lce",
      "label": "LCE",
      "parent_id": "vpe_4_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_4_ce",
      "label": "CE",
      "parent_id": "vpe_4_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_4_mctf_2",
      "label": "MCTF",
      "parent_id": "vpe_4_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": "mctf",
      "iq_override_id": "mctf_2",
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_4_sharpen",
      "label": "Sharpen",
      "parent_id": "vpe_4_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_full_pass",
      "label": "pass-3",
      "parent_id": "vpe",
      "kind": "group",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [],
      "outputs": [],
      "reason": null,
      "default_expanded": true
    },
    {
      "id": "vpe_full_pyrc",
      "label": "PYRC",
      "parent_id": "vpe_full_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in",
        "feedback"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_full_mctf_1",
      "label": "MCTF",
      "parent_id": "vpe_full_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": "mctf",
      "iq_override_id": "mctf_1",
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_full_lce",
      "label": "LCE",
      "parent_id": "vpe_full_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_full_ce",
      "label": "CE",
      "parent_id": "vpe_full_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_full_mctf_2",
      "label": "MCTF",
      "parent_id": "vpe_full_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": "mctf",
      "iq_override_id": "mctf_2",
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "vpe_full_sharpen",
      "label": "Sharpen",
      "parent_id": "vpe_full_pass",
      "kind": "operator",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [
        "out"
      ],
      "reason": "pyramid input unavailable",
      "default_expanded": false
    },
    {
      "id": "encoder",
      "label": "FFmpeg Encoder",
      "parent_id": "normal",
      "kind": "endpoint",
      "mode": "disabled",
      "execution_node_id": null,
      "module_id": null,
      "iq_override_id": null,
      "inputs": [
        "in"
      ],
      "outputs": [],
      "reason": null,
      "default_expanded": false
    }
  ],
  "iq_overrides": [
    {
      "id": "mctf_1",
      "module_id": "mctf"
    },
    {
      "id": "mctf_2",
      "module_id": "mctf"
    }
  ],
  "edges": [
    {
      "id": "normal_edge_0",
      "from": "raw_source",
      "to": "blc",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_1",
      "from": "blc",
      "to": "sbpc_horizontal",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_2",
      "from": "sbpc_horizontal",
      "to": "dbpc",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_3",
      "from": "dbpc",
      "to": "sbpc",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_4",
      "from": "sbpc",
      "to": "tintless",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_5",
      "from": "tintless",
      "to": "lsc",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_6",
      "from": "lsc",
      "to": "hr",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_7",
      "from": "hr",
      "to": "drc",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_8",
      "from": "drc",
      "to": "cac",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_9",
      "from": "cac",
      "to": "raw_nr",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_10",
      "from": "raw_nr",
      "to": "wbc",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_11",
      "from": "wbc",
      "to": "dem",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_12",
      "from": "dem",
      "to": "pfr",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_13",
      "from": "pfr",
      "to": "color_correction",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_14",
      "from": "color_correction",
      "to": "gamma",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_15",
      "from": "gamma",
      "to": "three_d_lut",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_16",
      "from": "three_d_lut",
      "to": "rgb2yuv",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_17",
      "from": "rgb2yuv",
      "to": "pyrd",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_18",
      "from": "vpe_16_sharpen",
      "to": "vpe_4_pyrc",
      "from_port": "out",
      "to_port": "feedback",
      "label": null
    },
    {
      "id": "normal_edge_19",
      "from": "vpe_4_sharpen",
      "to": "vpe_full_pyrc",
      "from_port": "out",
      "to_port": "feedback",
      "label": null
    },
    {
      "id": "normal_edge_20",
      "from": "vpe_full_sharpen",
      "to": "encoder",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_21",
      "from": "pyrd",
      "to": "vpe_full_pyrc",
      "from_port": "full",
      "to_port": "in",
      "label": "Full YUV"
    },
    {
      "id": "normal_edge_22",
      "from": "pyrd",
      "to": "vpe_4_pyrc",
      "from_port": "quarter",
      "to_port": "in",
      "label": "1/4 YUV"
    },
    {
      "id": "normal_edge_23",
      "from": "pyrd",
      "to": "vpe_16_pyrc",
      "from_port": "sixteenth",
      "to_port": "in",
      "label": "1/16 YUV"
    },
    {
      "id": "normal_edge_vpe_vpe_16_0",
      "from": "vpe_16_pyrc",
      "to": "vpe_16_mctf_1",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_16_1",
      "from": "vpe_16_mctf_1",
      "to": "vpe_16_lce",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_16_2",
      "from": "vpe_16_lce",
      "to": "vpe_16_ce",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_16_3",
      "from": "vpe_16_ce",
      "to": "vpe_16_mctf_2",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_16_4",
      "from": "vpe_16_mctf_2",
      "to": "vpe_16_sharpen",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_4_0",
      "from": "vpe_4_pyrc",
      "to": "vpe_4_mctf_1",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_4_1",
      "from": "vpe_4_mctf_1",
      "to": "vpe_4_lce",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_4_2",
      "from": "vpe_4_lce",
      "to": "vpe_4_ce",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_4_3",
      "from": "vpe_4_ce",
      "to": "vpe_4_mctf_2",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_4_4",
      "from": "vpe_4_mctf_2",
      "to": "vpe_4_sharpen",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_full_0",
      "from": "vpe_full_pyrc",
      "to": "vpe_full_mctf_1",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_full_1",
      "from": "vpe_full_mctf_1",
      "to": "vpe_full_lce",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_full_2",
      "from": "vpe_full_lce",
      "to": "vpe_full_ce",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_full_3",
      "from": "vpe_full_ce",
      "to": "vpe_full_mctf_2",
      "from_port": "out",
      "to_port": "in",
      "label": null
    },
    {
      "id": "normal_edge_vpe_vpe_full_4",
      "from": "vpe_full_mctf_2",
      "to": "vpe_full_sharpen",
      "from_port": "out",
      "to_port": "in",
      "label": null
    }
  ]
} as const;
