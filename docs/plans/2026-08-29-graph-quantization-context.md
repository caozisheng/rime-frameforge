# Design Context Notes

## Scope
User requests graph-level Node inspector configuration tree, global/per-module Rime.Q controls, simplified quantization contract, rime-quant rewrite, and defaults for BLC/WBC/DEM/RGB2YUV.

## Existing repository
- Rust workspace: `rime-core`, `rime-wasm`, `rime-dng`, `rime-isp`, `rime-quant`, Tauri desktop.
- React UI under `apps/desktop/src` with generated graph presentation and Node inspector.
- Normal Graph currently read-only; Minimal Graph executable.
- Existing quantization crate already has profile, quantize, dither modules and tests.
- Existing design/progress docs are authoritative for current architecture.

## Workflow constraints
- Must obtain design approval before implementation.
- Implementation will need a test-first plan after approval.
