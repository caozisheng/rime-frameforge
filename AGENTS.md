# AGENTS.md — Module Development Conventions

This file is the complete, self-contained specification for module development in this repository. Follow it as written; when code and this file disagree, reconcile both in the same change.

## Layout and authority

- Rust is the single source of truth for operators, manifests, and graph topology. `crates/`: `rime-core` (manifest/DAG/lifecycle), `rime-dng` (DNG decode), `rime-isp` (VFE/VBE/VPE operators), `rime-quant` (Rime.Q), `rime-scene` (scene/photometric semantics), `rime-iq`, `rime-wasm` (control plane), `rime-native-gpu` (wgpu executor), `rime-cli`.
- Before adding any non-ISP-specific capability (graph algorithms, parsers, scheduling, serialization, UI widgets), evaluate a mature, license-compatible open-source crate first. Implement only ISP contracts and thin adapters.
- Rust edition 2024, MSRV 1.93, `unsafe_code = "forbid"`, clippy `all` + `pedantic` denied. No warning may land.

## Adding or changing an ISP operator

### File layout and naming

One directory per module under `crates/rime-isp/src/<vfe|vbe|vpe>/<module>/`:

```text
mod.rs                          # OperatorDefinition, static OPERATOR, trait impl
<op><method>.rs                 # MethodManifest for that method (e.g. dem04.rs)
<op><method>.wgsl               # module-owned shader; even identity bypass is local
<op><method>_preprocess.rs      # CPU parameter derivation; canonical no-op if none
<op><method>_postprocess.rs     # CPU result handling; canonical no-op if none
<module>_common.rs              # optional stateless pure helpers ONLY
```

- Method ids are two-digit strings (`"00"`); files are prefixed with the operator id (`wbc00.rs`, `color_reproduce00.rs`).
- Every method owns its pre/post entry points. Methods must NOT share preprocess/postprocess function pointers; shared code goes in `<module>_common.rs`, which must not declare manifests, select methods, or hold cross-frame state.
- IQ assets (`<op><method>_iq.rs`, `<op><method>_iq_default.yaml`) live inside the same module boundary as the method they tune.

### Contracts

- Implement `Operator` (`crates/rime-isp/src/operator.rs`): `definition`, `preprocess`, `shader(method)`, `postprocess`. `MethodManifest` binds method id, ports, `output_rime_q_profile`, parameter schema, `ShaderAsset` (source, entry point, bindings, workgroup size), and pre/post hooks — one per method.
- Uniform packets are fixed-capacity (`MAX_UNIFORM_BYTES = 64`); reject overflow at construction, never allocate per frame.
- `PreprocessContext` is the only place metadata (DNG fields, IQ lookups, axis values) is read. Compute consumes the frozen `ModuleParameterPacket` only; a shader must never re-query metadata, IQ tables, or caches.
- WGSL shaders read the actual CFA/Bayer phase from the packet; never hardcode RGGB or other camera specifics.

### Registration and scheduling

- Register a new operator: export `pub const OPERATOR`, list it in `vfe::OPERATORS` / `vbe::OPERATORS` (thus `NORMAL_OPERATORS`), and wire its ports into `build_normal_manifest()` in `crates/rime-isp/src/graph.rs`. Then regenerate web assets.
- Per frame the runtime runs all preprocess in topological order, then all compute, then all postprocess. All three stages of a node must resolve to the same selected `MethodManifest`.
- Raw source and encoder are endpoints; they do not implement `Operator`. Incomplete modules (PYRD, PYRC, MCTF, LCE, CE, Sharpen) stay disabled and out of the executable registry until shader, ports, and parameter schema are complete.
- Executors (`rime-native-gpu`, WebGPU worker) compile pipelines from the selected `MethodManifest` — never from a fused shader bypassing the registry.

### Failure policy

Unknown operator/method, missing shader, uniform size/schema mismatch, invalid metadata, or GPU binding mismatch returns a stable error. NEVER fall back to a fused shader, default metadata, CPU image compute, or silent identity. CLI/native error prefixes (`CLI_*`, `NATIVE_*`) are stable API.

## IQ parameters and tables

- Scene luminance, exposure bias, and ISO/gain are distinct axes; never merge them into one.
- Multi-axis effects declare exactly one principal axis (direct value LUT); other axes are independent 1-D factor LUTs multiplied in declared order. `knots` and `values` lengths must match; out-of-range clamps per schema; invalid tables are rejected at load.
- Scene labels are metadata for UI/tuning organization only — never `if label == ...` branches in shaders.
- Styles override by canonical module address (`<domain>[<instance>].<module>[<instance>]`, e.g. `vbe.dem`) with complete-table atomic replacement; no field-level YAML patching, no silent fallback on mismatched overrides.
- Three-layer model, kept separate: module default IQ asset (read-only, versioned), user tuning profile YAML, per-frame resolved parameter snapshots (audit only; never re-consumed as profile input).
- Parameter changes take effect atomically at frame boundaries; an executing module is never retro-written by new revisions.
- Hue invariant: only Gamma applies a per-channel transfer function; every other 1-D tone/LUT curve works in luminance domain and applies one gain to all three linear RGB channels.

## Rime.Q quantization

- Quantization happens ONLY at declared module output ports (`uX.Y`/`sX.Y` profiles); resources stay `f32`. There is no input profile — a module's input inherits the upstream output profile.
- `rime-quant` is the authority for profile parsing, defaults, and effective state; WGSL executes the validated plan. Rust and WGSL helpers must stay numerically identical.
- Preview performs an independent display-domain conversion; never render signed `s0.Y` values directly as canvas color.
- Quantization verification requires the same input through both the quantized and unquantized paths.

## GPU resource rules

- The RAW frame uploads once per frame; from upload to encoder the main chain is zero-host-copy. Full-resolution readback to CPU is allowed only for the requested final preview.
- No in-place compute on an input texture; no GPU copies that just move an input to an output texture.
- Intermediate textures come from the pool keyed by format/extent/usage; recycle only after fence completion and lease release. GPU texture formats in use: `r16uint`, `r32float`, `rgba32float`.
- Sequences use a fixed-capacity ring (default 2); never preload a full sequence. Graph dependencies inside a submission rely on queue order, not per-node CPU awaits.

## Input and scene crates

- `rime-dng` wraps `gamut-dng` behind an adapter returning `DecodedRawFrame` + normalized metadata. Never hold, pass, or serialize `gamut-dng` types outside the adapter.
- Photometric/scene semantics (APEX, EV, CCT, scene tags) belong in `rime-scene` — not `rime-isp`, `rime-dng`, or `rime-core`.

## Web and desktop surface

- Graph topology is fixed at build time: the UI renders a read-only DAG; no runtime node add/remove/connect.
- The WASM control plane owns lifecycle, revision state, and serial command ordering; it does no per-pixel work. TypeScript coordinates; it does not redefine contracts.
- Parameter edits stay draft until Apply or Run commits them at a frame boundary.

## Workflow and verification

- Design first: for non-trivial module work, write a design note covering algorithm, dataflow, parameter semantics, resource contract, and verification criteria, and get it approved before implementation; follow with an implementation record.
- After touching operators, methods, ports, or topology, run `npm run generate:manifest` and commit the regenerated assets in the same change.
- Before handing off, all of the following must pass:

```powershell
npm test
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
npx tsc -b
```

- Renames are clean cutovers: update id, label, directory, files, parameters, generated assets, and every call site in one change (see `color_reproduce` for the pattern).

## Test DNG fixtures

- DNG tests and smoke verification use `pipeline/normal/P1020601.dng` (GH5S). The path is ignored; NEVER commit the fixture or change the CI download configuration.
- Obtain the fixture from the canonical `essentials-for-ci` release, same source as CI:

```powershell
curl --fail --location --retry 3 --create-dirs --output pipeline/normal/P1020601.dng https://github.com/caozisheng/rime-frameforge/releases/download/essentials-for-ci/P1020601.dng
```

- Verify before use — SHA-256: `ed8ebc01903b36bd6c86287d960c2244059105a6ae7b661a0efcfea1acdf2848`.
