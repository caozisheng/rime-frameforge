export const normalGraphQuantization = {
  "graph_id": "normal",
  "enabled": true,
  "modules": [
    {
      "module_id": "blc",
      "output_enabled": true,
      "output_profile": "u0.14",
      "clip_type": "truncate"
    },
    {
      "module_id": "sbpc_horizontal",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "dbpc",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "sbpc",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "tintless",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "lsc",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "hr",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "drc",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "cac",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "raw_nr",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "wbc",
      "output_enabled": true,
      "output_profile": "u0.12",
      "clip_type": "truncate"
    },
    {
      "module_id": "dem",
      "output_enabled": true,
      "output_profile": "u0.12",
      "clip_type": "truncate"
    },
    {
      "module_id": "pfr",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "color_correction",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "gamma",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "three_d_lut",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    },
    {
      "module_id": "rgb2yuv",
      "output_enabled": true,
      "output_profile": "u0.10",
      "clip_type": "truncate"
    }
  ]
} as const;
