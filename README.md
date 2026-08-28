# Rime FrameForge

Rime FrameForge is a Windows-focused desktop workbench for ISP architecture modeling, graph inspection, and GPU-backed RAW frame experiments.

The repository keeps the ISP topology, executable manifest, runtime lifecycle, and WebGPU execution contracts explicit. Rust is the source of truth for operators and graph assets; TypeScript renders the desktop UI and coordinates GPU execution; WebAssembly owns the runtime state machine.

## Current status

The current application is a **Normal Graph** workbench with a real GPU execution path for the active operators and an architecture view for the broader VFE/VBE/VPE pipeline.

## Architecture

```text
DNG / RAW file
      │
      ▼
Tauri native commands ── inspect metadata / read RAW samples
      │
      ▼
React desktop UI ── Worker bridge ── Web Worker
                                      │
                                      ├─ WASM runtime authority
                                      ├─ WebGPU executor
                                      └─ GPU resource and transfer audits
```

The main image path uploads RAW once and keeps subsequent processing GPU-resident. WGSL shaders perform per-pixel operations. The WASM control plane manages graph lifecycle and revision state; it does not perform per-pixel image processing.

## Workspace layout

```text
apps/desktop/          React UI, Worker bridge, and Tauri shell
crates/rime-core/      Manifest, DAG, diagnostics, lifecycle
crates/rime-dng/       DNG metadata and RAW frame decoding
crates/rime-isp/       VFE/VBE/VPE operator definitions and graph builders
crates/rime-quant/     Fixed-grid f32 quantization and deterministic dither
crates/rime-wasm/      WASM runtime control plane
pipeline/normal/       Fixed smoke RAW asset and metadata
web/                   TypeScript contracts, runtime, and WebGPU code
```

Rust graph and operator definitions are the canonical source. Files under `web/src/generated/` are generated and must not be hand-edited. The `pipeline/normal/` directory contains only runtime smoke input data.

## Requirements

The primary development target is Windows 11 x64.

- Node.js 22+
- npm 10+
- Rust stable with the workspace Rust 1.93 requirement
- `wasm-pack` 0.15+
- Rust target `wasm32-unknown-unknown`
- Windows WebView2 Runtime
- Windows MSVC C++ Build Tools
- WebGPU-capable browser/GPU supporting `r16uint`, `r32float`, and `rgba32float`

## Setup

From the repository root:

```powershell
npm install
cargo fetch
rustup target add wasm32-unknown-unknown
```

## Development

Run the desktop web surface:

```powershell
npm run dev -w @rime/desktop
```

Open `http://127.0.0.1:1420` in a WebGPU-capable browser.

Run the Tauri application:

```powershell
npm run tauri:dev
```

## Generate assets

After changing Rust operators, methods, ports, or graph topology, regenerate the checked-in Web assets:

```powershell
npm run generate:manifest
```
This updates:

- `web/src/generated/normal_manifest.generated.ts`
- `web/src/generated/top_graph.generated.ts`
- `web/src/generated/normal_graph.generated.ts`

## Testing and validation

```powershell
npm test
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
npx tsc -b
```

The tests cover graph contracts, generated topology, method selection, DNG metadata, runtime lifecycle, serial command ordering, GPU resource lifecycle, transfer audits, panel layout, and graph edge routing.

## Build

Build the desktop web frontend:

```powershell
npm run build
```

Build the Windows Tauri application:

```powershell
npm run tauri:build

```

## CI and releases

GitHub Actions runs Rust and frontend checks for pushes to `main` and pull requests:

```text
.github/workflows/ci.yml
```

After updating all project version fields to the same `X.Y.Z` value, create and push a matching tag. The tag starts the Windows release workflow:

```powershell
pwsh .github/scripts/release.ps1 0.1.1 -Push
```

This validates the Cargo, npm, and Tauri versions, pushes `main` and `v0.1.1`, builds the Windows Tauri bundles, and publishes the MSI/NSIS/EXE artifacts to a GitHub Release.

The local build entry point is:

```powershell
pwsh .github/scripts/build-windows.ps1
```

The generated `release-artifacts/` directory is a local/CI output directory and is not committed.

## Current limitations

The current implementation does not yet provide:

- Full algorithm implementations for every bypassed VFE/VBE/VPE operator
- Real VPE execution in the Normal Graph
- Full temporal history and cross-frame DAG execution
- General-purpose IQ table editing UI
- Selected/Compare Preview workflows
- WebCodecs HEVC Main10 encoding, MP4 muxing, or video export
- CPU image fallback or CPU readback in the main image path
