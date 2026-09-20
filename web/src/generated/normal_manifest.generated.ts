export const normalManifest = {
  "schema_version": 1,
  "graph_id": "normal",
  "graph_kind": "video-isp/normal",
  "manifest_hash": "cf422c745b7c3bd84f4ab673f7c057dbec8032c97aaf1ea407fb58ec58da2e11",
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
      "id": "tintless",
      "display_name": "TINTLESS",
      "shader_entry": "tintless_main",
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
      "statistics_inputs": [
        {
          "id": "lc-stat",
          "schema": {
            "kind": "lcst",
            "average_rggb": {
              "width": 64,
              "height": 48,
              "channels": 4,
              "scalar": "f32"
            },
            "luma_histogram": {
              "width": 16,
              "height": 16,
              "channels": 16,
              "scalar": "u32"
            }
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "tintless_main",
          "parameters": [
            "source_extent",
            "mesh_extent",
            "cfa_pattern",
            "gain_clamp",
            "cold_start",
            "gain_mesh"
          ]
        }
      ]
    },
    {
      "id": "lsc",
      "display_name": "LSC",
      "shader_entry": "lsc_main",
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
          "shader_entry": "lsc_main",
          "parameters": [
            "mesh_count",
            "cfa_phase",
            "gain_mesh_headers",
            "gain_mesh_entries"
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
            "blue_gain",
            "enable_highlight_recovery",
            "hr_gain"
          ]
        }
      ]
    },
    {
      "id": "drc",
      "display_name": "DRC",
      "shader_entry": "drc_combine_global_main",
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
      "statistics_inputs": [
        {
          "id": "lc-stat",
          "schema": {
            "kind": "lcst",
            "average_rggb": {
              "width": 64,
              "height": 48,
              "channels": 4,
              "scalar": "f32"
            },
            "luma_histogram": {
              "width": 16,
              "height": 16,
              "channels": 16,
              "scalar": "u32"
            }
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "drc_combine_global_main",
          "parameters": [
            "drc_gain",
            "hr_gain",
            "knee",
            "amplifier",
            "enable_details_amplify",
            "luma_guard",
            "min_ratio",
            "max_ratio",
            "level_count",
            "feature_flags",
            "analysis_wbc_gains",
            "global_tone_lut"
          ]
        },
        {
          "method": "01",
          "shader_entry": "drc_combine_local_main",
          "parameters": [
            "drc_gain",
            "hr_gain",
            "knee",
            "amplifier",
            "enable_details_amplify",
            "luma_guard",
            "min_ratio",
            "max_ratio",
            "level_count",
            "feature_flags",
            "analysis_wbc_gains",
            "global_tone_lut",
            "local_tone_lut"
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
      "id": "color_reproduce",
      "display_name": "Color Reproduce",
      "shader_entry": "color_reproduce_main",
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
          "shader_entry": "color_reproduce_main",
          "parameters": [
            "sensor_to_prophoto",
            "hs_lut",
            "prophoto_to_srgb",
            "gamma",
            "gamma_lut"
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
    },
    {
      "id": "lcst",
      "display_name": "LCST",
      "shader_entry": "lcst_average_main",
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
      "outputs": [],
      "statistics_outputs": [
        {
          "id": "lc-stat",
          "schema": {
            "kind": "lcst",
            "average_rggb": {
              "width": 64,
              "height": 48,
              "channels": 4,
              "scalar": "f32"
            },
            "luma_histogram": {
              "width": 16,
              "height": 16,
              "channels": 16,
              "scalar": "u32"
            }
          }
        }
      ],
      "default_method": "00",
      "methods": [
        {
          "method": "00",
          "shader_entry": "lcst_average_main",
          "parameters": [
            "width",
            "height",
            "cfa_pattern",
            "d50_gains"
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
        "node_id": "raw_nr",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_5",
      "from": {
        "node_id": "raw_nr",
        "port_id": "out"
      },
      "to": {
        "node_id": "tintless",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_6",
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
      "id": "normal_edge_7",
      "from": {
        "node_id": "lsc",
        "port_id": "out"
      },
      "to": {
        "node_id": "wbc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_8",
      "from": {
        "node_id": "wbc",
        "port_id": "out"
      },
      "to": {
        "node_id": "drc",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_9",
      "from": {
        "node_id": "drc",
        "port_id": "out"
      },
      "to": {
        "node_id": "dem",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_10",
      "from": {
        "node_id": "dem",
        "port_id": "out"
      },
      "to": {
        "node_id": "color_reproduce",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_11",
      "from": {
        "node_id": "color_reproduce",
        "port_id": "out"
      },
      "to": {
        "node_id": "rgb2yuv",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_sbpc_lcst",
      "from": {
        "node_id": "sbpc",
        "port_id": "out"
      },
      "to": {
        "node_id": "lcst",
        "port_id": "in"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_lcst_tintless",
      "from": {
        "node_id": "lcst",
        "port_id": "lc-stat"
      },
      "to": {
        "node_id": "tintless",
        "port_id": "lc-stat"
      },
      "frame_delay": 0
    },
    {
      "id": "normal_edge_lcst_drc",
      "from": {
        "node_id": "lcst",
        "port_id": "lc-stat"
      },
      "to": {
        "node_id": "drc",
        "port_id": "lc-stat"
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
      "node_id": "color_reproduce",
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
