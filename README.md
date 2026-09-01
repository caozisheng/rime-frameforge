# Rime FrameForge

Rime FrameForge converts DNG/RAW frames into preview images and compressed video while keeping the graph topology, operator contracts, runtime lifecycle, and GPU execution behavior explicit.

The repository contains two execution paths:

- **Desktop interactive path:** React/Tauri + WASM runtime authority + WebGPU Worker. This is the v0.1.3-compatible path used for high-rate preview and supports the current browser WebGPU executor.
- **Rust native path:** `rime-dng` + `rime-native-gpu` + `rime-cli`. It provides headless file-to-file orchestration and explicit native GPU readback. It is not the NVENC zero-copy backend yet.

Rust remains the source of truth for operators and graph assets. TypeScript renders the desktop UI and coordinates the WebGPU path; WebAssembly owns the existing runtime state machine and revision contract.

## Architecture

```text
Desktop interactive preview
DNG → Tauri metadata/RAW command → React → Worker
                                      ├─ WASM runtime authority
                                      └─ WebGPU graph → GPU texture → canvas

Headless native/CLI path
DNG → rime-dng DecodedRawFrame → rime-native-gpu wgpu graph
                                  └─ readback → PNG / CPU FFmpeg fallback
```

The v0.1.3 desktop path uploads RAW once and keeps subsequent processing GPU-resident. The native readback path is intended for CLI, native verification, and future encoder integration; it currently performs a final GPU→CPU readback and therefore is not the desktop preview default.

The WASM control plane manages graph lifecycle and revision state; it does not perform per-pixel image processing.

## Workspace layout

```text
apps/desktop/           React UI, Worker bridge, and Tauri shell
crates/rime-core/       Manifest, DAG, diagnostics, lifecycle
crates/rime-dng/        DNG metadata and RAW frame decoding
crates/rime-isp/        VFE/VBE/VPE operator definitions and graph builders
crates/rime-quant/      Fixed-grid f32 quantization and deterministic dither
crates/rime-wasm/       WASM runtime control plane
crates/rime-native-gpu/ Rust wgpu graph/readback backend and bounded frame ring
crates/rime-cli/        Headless `rime-frameforge` command-line interface
pipeline/normal/        Fixed smoke RAW asset and metadata
web/                    TypeScript contracts, runtime, and WebGPU code
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

## CLI

The Rust CLI binary is named `rime-frameforge`. Run it from the repository root during development:

```powershell
cargo run --offline -p rime-cli -- --help
```

### `inspect`

Inspect one DNG using the existing `rime-dng` frame and metadata contract:

```powershell
rime-frameforge inspect input.dng
rime-frameforge inspect input.dng --json
```

`--json` emits one machine-readable object containing dimensions, CFA, camera model, metadata hash, and RAW digest.

### `render`

Render one DNG through the Rust native `wgpu` readback backend into PNG:

```powershell
rime-frameforge render input.dng --output output.png
rime-frameforge render input.dng --output output.png --json --print-resolved-config
```

### `render-sequence`

Select any DNG member. The CLI scans only its direct parent directory, filters `.dng` case-insensitively, and applies the existing natural filename ordering:

```powershell
rime-frameforge render-sequence P1020601.dng --output output.mp4 --codec hevc --fps 24
rime-frameforge render-sequence P1020601.dng --output output.mp4 --dry-run --json --print-resolved-config
```

The sequence scheduler uses a fixed-capacity ring, default capacity `2`, and never preloads the full sequence. The current video path is explicitly `cpu_readback`: native GPU output is read back and sent to a validated FFmpeg process. It is not NVENC texture zero-copy.

### `graph validate` and `graph show`

Validate or inspect the Rust-generated Normal Graph:

```powershell
rime-frameforge graph validate
rime-frameforge graph validate --graph normal
rime-frameforge graph show
```

Only `normal` is currently accepted by the native CLI.

### Common render options

| Option | Default | Description |
| --- | --- | --- |
| `--output <PATH>` | required | Output PNG/video path |
| `--json` | off | Emit machine-readable progress on stdout |
| `--quiet` | off | Suppress progress output |
| `--progress <FORMAT>` | `jsonl` | Progress format; currently JSONL |
| `--dry-run` | off | Validate and resolve without producing output |
| `--print-resolved-config` | off | Print resolved graph/backend configuration |
| `--graph-config <PATH>` | none | Load and validate graph quantization config |
| `--codec <CODEC>` | `h264` | Sequence codec: `h264` or `hevc` |
| `--fps <N>` | `24` | Sequence frame rate |

JSONL events include `frame_started`, `frame_completed`, `dry_run`, and `completed`. Errors use stable `CLI_*` / `NATIVE_*` prefixes.

The native readback path currently rejects unsupported non-default graph configurations instead of silently ignoring DEM method or Rime.Q settings. The desktop interactive preview continues to use the v0.1.3 WebGPU Worker path for performance and feature parity.

## Generate assets

After changing Rust operators, methods, ports, or graph topology, regenerate the checked-in Web assets:

```powershell
npm run generate:manifest
```

This updates:

- `web/src/generated/normal_manifest.generated.ts`
- `web/src/generated/top_graph.generated.ts`
- `web/src/generated/normal_graph.generated.ts`
- `web/src/generated/normal_quantization.generated.ts`

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

After updating all project version fields to the same `X.Y.Z` value, create and push a matching tag. The tag starts the Windows release workflow.

For the current release:

```powershell
pwsh .github/scripts/release.ps1 0.1.4 -Push
```

This validates the Cargo, npm, and Tauri versions, pushes `main` and `v0.1.4`, builds the Windows Tauri bundles, and publishes the MSI/NSIS/EXE artifacts to a GitHub Release.

The local build entry point is:

```powershell
pwsh .github/scripts/build-windows.ps1
```

The generated `release-artifacts/` directory is a local/CI output directory and is not committed.
