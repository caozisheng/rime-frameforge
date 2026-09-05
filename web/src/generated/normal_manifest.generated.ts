export const normalManifest = {
  "schema_version": 1,
  "graph_id": "normal",
  "graph_kind": "video-isp/normal",
  "manifest_hash": "ae4c6f1bae91dd2cb0a7bea10240587b3f73d87f9ef8f64cebbe302274656f63",
  "nodes": [
    {
      "id": "raw_source",
      "display_name": "RAW Source",
      "shader_entry": null,
      "inputs": [],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_sensor",
          "format": "r16_uint",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "fixed_asset",
      "methods": []
    },
    {
      "id": "blc",
      "display_name": "BLC",
      "shader_entry": "blc_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_sensor",
          "format": "r16_uint",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "blc_main",
          "parameters": [
            "black_level",
            "white_level",
            "width",
            "height"
          ]
        }
      ]
    },
    {
      "id": "sbpc_horizontal",
      "display_name": "SBPC-H",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "dbpc",
      "display_name": "DBPC",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "sbpc",
      "display_name": "SBPC",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "tintless",
      "display_name": "TINTLESS",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "lsc",
      "display_name": "LSC",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "hr",
      "display_name": "HR",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "drc",
      "display_name": "DRC",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "cac",
      "display_name": "CAC",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "raw_nr",
      "display_name": "RAW-NR",
      "shader_entry": "identity_r32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_r32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "wbc",
      "display_name": "WBC",
      "shader_entry": "wbc_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "wbc_main",
          "parameters": [
            "red_gain",
            "green_gain",
            "blue_gain"
          ]
        }
      ]
    },
    {
      "id": "dem",
      "display_name": "DEM",
      "shader_entry": "demosaic_bilinear_main",
      "inputs": [
        {
          "id": "in",
          "domain": "raw_bayer_rime_q",
          "format": "r32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "linear_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "demosaic_bilinear_main",
          "parameters": [
            "cfa_pattern"
          ]
        },
        {
          "method": "01",
          "shader_entry": "demosaic_mhc_main",
          "parameters": [
            "cfa_pattern"
          ]
        },
        {
          "method": "02",
          "shader_entry": "demosaic_ppg_main",
          "parameters": [
            "cfa_pattern"
          ]
        },
        {
          "method": "03",
          "shader_entry": "demosaic_vng_main",
          "parameters": [
            "cfa_pattern",
            "vng_threshold"
          ]
        },
        {
          "method": "04",
          "shader_entry": "demosaic_ahd_main",
          "parameters": [
            "cfa_pattern",
            "ahd_l_threshold",
            "ahd_c_threshold_sq"
          ]
        }
      ]
    },
    {
      "id": "pfr",
      "display_name": "PFR",
      "shader_entry": "identity_rgba32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "linear_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "linear_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_rgba32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "color_correction",
      "display_name": "CCM 8 x 3 x 3",
      "shader_entry": "color_correction_main",
      "inputs": [
        {
          "id": "in",
          "domain": "linear_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "linear_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "color_correction_main",
          "parameters": [
            "ccm"
          ]
        }
      ]
    },
    {
      "id": "gamma",
      "display_name": "Gamma",
      "shader_entry": "gamma_main",
      "inputs": [
        {
          "id": "in",
          "domain": "linear_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "encoded_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "gamma_main",
          "parameters": [
            "gamma",
            "gamma_lut"
          ]
        }
      ]
    },
    {
      "id": "three_d_lut",
      "display_name": "3D LUT 17³",
      "shader_entry": "identity_rgba32_main",
      "inputs": [
        {
          "id": "in",
          "domain": "encoded_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "encoded_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "identity_rgba32_main",
          "parameters": [
            "identity"
          ]
        }
      ]
    },
    {
      "id": "rgb2yuv",
      "display_name": "RGB2YUV",
      "shader_entry": "rgb2yuv_main",
      "inputs": [
        {
          "id": "in",
          "domain": "encoded_rgb",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "outputs": [
        {
          "id": "out",
          "domain": "yuv",
          "format": "rgba32_float",
          "extent": {
            "width": 32,
            "height": 24
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "rgb2yuv_main",
          "parameters": [
            "bt709"
          ]
        }
      ]
    }
  ],
  "edges": [
    {
      "id": "normal_edge_0",
      "from": {
        "node_id": "raw_source",
        "port_id": "out"
      },
      "to": {
        "node_id": "blc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_1",
      "from": {
        "node_id": "blc",
        "port_id": "out"
      },
      "to": {
        "node_id": "sbpc_horizontal",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_2",
      "from": {
        "node_id": "sbpc_horizontal",
        "port_id": "out"
      },
      "to": {
        "node_id": "dbpc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_3",
      "from": {
        "node_id": "dbpc",
        "port_id": "out"
      },
      "to": {
        "node_id": "sbpc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_4",
      "from": {
        "node_id": "sbpc",
        "port_id": "out"
      },
      "to": {
        "node_id": "tintless",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_5",
      "from": {
        "node_id": "tintless",
        "port_id": "out"
      },
      "to": {
        "node_id": "lsc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_6",
      "from": {
        "node_id": "lsc",
        "port_id": "out"
      },
      "to": {
        "node_id": "hr",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_7",
      "from": {
        "node_id": "hr",
        "port_id": "out"
      },
      "to": {
        "node_id": "drc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_8",
      "from": {
        "node_id": "drc",
        "port_id": "out"
      },
      "to": {
        "node_id": "cac",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_9",
      "from": {
        "node_id": "cac",
        "port_id": "out"
      },
      "to": {
        "node_id": "raw_nr",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_10",
      "from": {
        "node_id": "raw_nr",
        "port_id": "out"
      },
      "to": {
        "node_id": "wbc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_11",
      "from": {
        "node_id": "wbc",
        "port_id": "out"
      },
      "to": {
        "node_id": "dem",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_12",
      "from": {
        "node_id": "dem",
        "port_id": "out"
      },
      "to": {
        "node_id": "pfr",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_13",
      "from": {
        "node_id": "pfr",
        "port_id": "out"
      },
      "to": {
        "node_id": "color_correction",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_14",
      "from": {
        "node_id": "color_correction",
        "port_id": "out"
      },
      "to": {
        "node_id": "gamma",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_15",
      "from": {
        "node_id": "gamma",
        "port_id": "out"
      },
      "to": {
        "node_id": "three_d_lut",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_16",
      "from": {
        "node_id": "three_d_lut",
        "port_id": "out"
      },
      "to": {
        "node_id": "rgb2yuv",
        "port_id": "in"
      },
      "frame_delay": 0
    }
  ],
  "preview_outputs": [
    {
      "node_id": "rgb2yuv",
      "port_id": "out",
      "domain": "yuv",
      "format": "rgba32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "rgba",
      "presentation": "yuv"
    },
    {
      "node_id": "three_d_lut",
      "port_id": "out",
      "domain": "encoded_rgb",
      "format": "rgba32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "rgba",
      "presentation": "rgb"
    },
    {
      "node_id": "gamma",
      "port_id": "out",
      "domain": "encoded_rgb",
      "format": "rgba32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "rgba",
      "presentation": "rgb"
    },
    {
      "node_id": "color_correction",
      "port_id": "out",
      "domain": "linear_rgb",
      "format": "rgba32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "rgba",
      "presentation": "rgb"
    },
    {
      "node_id": "pfr",
      "port_id": "out",
      "domain": "linear_rgb",
      "format": "rgba32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "rgba",
      "presentation": "rgb"
    },
    {
      "node_id": "dem",
      "port_id": "out",
      "domain": "linear_rgb",
      "format": "rgba32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "rgba",
      "presentation": "rgb"
    },
    {
      "node_id": "wbc",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "raw_nr",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "cac",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "drc",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "hr",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "lsc",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "tintless",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "sbpc",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "dbpc",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "sbpc_horizontal",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "blc",
      "port_id": "out",
      "domain": "raw_bayer_rime_q",
      "format": "r32_float",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "normalized",
      "channel_layout": "scalar",
      "presentation": "raw_gray"
    },
    {
      "node_id": "raw_source",
      "port_id": "out",
      "domain": "raw_bayer_sensor",
      "format": "r16_uint",
      "extent": {
        "width": 32,
        "height": 24
      },
      "range": "sensor_code",
      "channel_layout": "cfa",
      "presentation": "raw_gray"
    }
  ]
} as const;

export type NormalManifest = typeof normalManifest;
