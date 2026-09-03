# DNG AsShotWhiteXY White-Balance Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Decode DNG files that provide `AsShotWhiteXY`, compute normalized RGB white-balance gains during preprocess, and make both WBC shader paths consume those gains uniformly.

**Architecture:** Pin `gamut-dng` to the upstream revision that already models `AsShotWhiteXY` and derives a camera-native neutral after calibration. Keep raw DNG source fields optional in `rime-dng` metadata. Add a project-owned preprocess function that chooses `AsShotNeutral` first, otherwise converts `AsShotWhiteXY` with `ColorMatrix2` then `ColorMatrix1`, computes green-normalized RGB gains, and feeds those gains into the WBC WGSL and native fused WGSL uniforms.

**Tech Stack:** Rust 2024, Cargo workspace, `gamut-dng`, WGPU/WGSL, Vitest/TypeScript descriptors, local GH5S DNG fixtures.

---

### Task 1: Pin DNG decoder support for AsShotWhiteXY

**Files:**
- Modify: `crates/rime-dng/Cargo.toml:8-12`
- Modify: `Cargo.lock` through Cargo resolution
- Test: `crates/rime-dng/tests/adapter.rs`

**Step 1: Write the failing test**

Add a decoder regression test that locates a local DNG containing tag 50729 (`AsShotWhiteXY`) and no tag 50728 (`AsShotNeutral`), then asserts `DngReader::decode_file` succeeds and exposes the WhiteXY source. If the local fixture set has no Pixel fixture, add a small deterministic in-memory DNG fixture using the existing `gamut-dng` test construction pattern; do not commit camera data.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rime-dng --test adapter pixel_white_xy_dng_decodes -- --nocapture
```

Expected: FAIL because the locked `gamut-dng 1.0.0` decoder reports missing `AsShotNeutral`.

**Step 3: Write minimal implementation**

Change the `gamut-dng` dependency from the crates.io-only declaration to the exact upstream git revision `6a75ec4af082289e372dddba8cfa49396f2068fe`, package `gamut-dng`. Adapt only the `rime-dng` call sites required by the upstream API, especially optional profile handling and the new `as_shot_white_xy()` accessor. Preserve source metadata in project-owned fields; do not replace WhiteXY with a synthetic inspector value.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p rime-dng --test adapter pixel_white_xy_dng_decodes -- --nocapture
```

Expected: PASS, with the WhiteXY-only file decoded successfully.

**Step 5: Commit**

```bash
git add crates/rime-dng/Cargo.toml Cargo.lock crates/rime-dng/tests/adapter.rs
git commit -m "fix(dng): decode AsShotWhiteXY profiles"
```

---

### Task 2: Preserve optional white-balance source metadata

**Files:**
- Modify: `crates/rime-dng/src/lib.rs:37-72,168-222,307-353`
- Modify: `apps/desktop/src-tauri/src/dng_command.rs:19-52,243-277`
- Modify: `web/src/runtime/worker-bridge.ts` at `DngMetadataDescriptor`
- Modify: `web/src/components/dng-metadata.ts` calibration tree
- Test: `crates/rime-dng/tests/inspector_metadata.rs`
- Test: `apps/desktop/src-tauri/tests/dng_descriptor.rs`
- Test: `web/tests/dng-metadata.test.ts`

**Step 1: Write the failing tests**

Change metadata tests to assert:

- existing GH5S data has `as_shot_neutral: Some([..])` and no WhiteXY source;
- WhiteXY-only data has `as_shot_neutral: None` and `as_shot_white_xy: Some([x, y])`;
- serialized descriptors preserve both optional fields with camelCase names.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rime-dng --test inspector_metadata
cargo test -p rime-desktop --test dng_descriptor
npm test -- web/tests/dng-metadata.test.ts
```

Expected: FAIL because the current contract stores `as_shot_neutral` as a required array and has no `as_shot_white_xy` field.

**Step 3: Write minimal implementation**

Change Rust metadata and Tauri descriptor fields to `Option` values. Populate them from the upstream `CameraProfile`: `as_shot_neutral()` remains available for the derived/explicit neutral used by decoder internals, while `as_shot_white_xy()` identifies the stored source form. Update TypeScript interfaces, test fixtures, and inspector tree rendering so null values are handled without hiding the other calibration fields.

**Step 4: Run tests to verify they pass**

Run the same three commands. Expected: PASS with source metadata represented accurately.

**Step 5: Commit**

```bash
git add crates/rime-dng apps/desktop/src-tauri web/src web/tests
 git commit -m "feat(dng): expose optional white balance metadata"
```

---

### Task 3: Add preprocess white-balance gain calculation

**Files:**
- Modify: `crates/rime-isp/src/lib.rs`
- Create or modify: `crates/rime-isp/src/preprocess.rs`
- Modify: `crates/rime-isp/Cargo.toml` only if a dependency is genuinely required
- Test: `crates/rime-isp/tests/preprocess.rs`

**Step 1: Write the failing tests**

Add focused tests for:

- explicit `AsShotNeutral` wins when both sources exist;
- WhiteXY uses `ColorMatrix2` when present;
- WhiteXY falls back to `ColorMatrix1` when `ColorMatrix2` is absent;
- conversion follows `X=x/y`, `Y=1`, `Z=(1-x-y)/y`, matrix multiply, then divide by camera green;
- reciprocal gains are normalized by green, making green gain exactly 1;
- invalid coordinates, matrices, neutral values, and non-finite gains return stable errors.

Use synthetic matrices and values so expected results are deterministic and independent of a camera fixture.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rime-isp --test preprocess -- --nocapture
```

Expected: FAIL because no preprocess gain API exists.

**Step 3: Write minimal implementation**

Add a project-owned API with an explicit input contract containing optional neutral/WhiteXY and both color matrices, plus:

```rust
pub struct WhiteBalanceGains {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}
```

Implement source precedence, WhiteXY conversion, finite/positive validation, reciprocal calculation, and green normalization. Keep this computation independent from GPU and UI crates. Do not clone large frame buffers or compute gains per pixel.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p rime-isp --test preprocess -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/rime-isp/src crates/rime-isp/tests/preprocess.rs
git commit -m "feat(isp): compute metadata white balance gains"
```

---

### Task 4: Wire RGB gains into the WBC shader contract

**Files:**
- Modify: `crates/rime-isp/src/vbe/white_balance/white_balance_00.wgsl:1-14`
- Modify: `crates/rime-native-gpu/src/wgpu_backend.rs:14-27,44-52,148-160`
- Modify: `crates/rime-native-gpu/Cargo.toml` only if needed for the preprocess crate dependency
- Test: `crates/rime-isp/tests/operator_methods.rs`
- Test: `crates/rime-native-gpu/tests/readback.rs`

**Step 1: Write the failing tests**

Add/adjust tests that verify:

- WBC remains declared with exactly `red_gain green_gain blue_gain` parameters;
- native render parameters contain the computed gains;
- a small 2×2 CFA sample produces channel-scaled output using supplied gains rather than fixed red/blue constants.

The shader source test must assert observable behavior through the existing validation/render path where possible, not only string matching.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rime-isp --test operator_methods
cargo test -p rime-native-gpu --test readback -- --nocapture
```

Expected: FAIL because the WBC shader and fused shader use hard-coded gains and `FusedParams` has no gain fields.

**Step 3: Write minimal implementation**

Add three `f32` gain fields to the WBC uniform contract with WGSL alignment accounted for. In the regular WBC shader, map CFA channel 0/1/2 to red/green/blue gain and multiply the sample. In the native fused shader, use the same mapping and write the preprocessed values into `FusedParams`. The native path must call the shared preprocess API instead of duplicating conversion math.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p rime-isp --test operator_methods
cargo test -p rime-native-gpu --test readback -- --nocapture
```

Expected: PASS. GPU tests may skip only when no adapter is available, matching existing repository behavior.

**Step 5: Commit**

```bash
git add crates/rime-isp crates/rime-native-gpu
 git commit -m "feat(wbc): use preprocessed RGB gains"
```

---

### Task 5: Regenerate assets and update integration contracts

**Files:**
- Modify: `web/src/generated/normal_manifest.generated.ts`
- Modify: `web/src/generated/normal_graph.generated.ts` only if generation changes it
- Modify: `web/src/generated/normal_quantization.generated.ts` only if generation changes it
- Modify: affected descriptor fixtures and tests

**Step 1: Write the failing integration assertion**

Add an integration assertion that generated WBC parameters remain exactly `red_gain`, `green_gain`, and `blue_gain`, and that no legacy fixed-gain parameter names are generated.

**Step 2: Run it to verify it fails if generation is stale**

Run:

```bash
npm run generate:manifest
cargo test -p rime-isp --test normal_manifest
```

Expected: the test fails or generated diff shows stale WBC assets if the generated contract is not synchronized.

**Step 3: Regenerate and adjust only required consumers**

Run the repository generator, inspect the generated diff, and keep generated output consistent with the source operator definition. Update TypeScript consumers only where optional metadata or parameter schemas changed.

**Step 4: Run integration checks**

Run:

```bash
cargo test -p rime-isp --test normal_manifest
npm test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add web/src/generated web/src web/tests crates/rime-isp/tests
 git commit -m "chore: regenerate white balance contracts"
```

---

### Task 6: Verify DNG and GPU behavior end to end

**Files:**
- Test: `crates/rime-dng/tests/adapter.rs`
- Test: `crates/rime-isp/tests/preprocess.rs`
- Test: `crates/rime-native-gpu/tests/readback.rs`
- Test: existing CLI/native smoke paths

**Step 1: Prepare local fixture paths**

Use only DNG fixtures from `C:\Users\zisheng\Documents\cao\99_data\isp\pana_gh5s`. If a test requires `pipeline/normal/P1020601.dng`, temporarily copy the local GH5S fixture to that ignored path; do not commit it or change CI download configuration.

**Step 2: Run targeted Rust tests**

```bash
cargo test -p rime-dng
cargo test -p rime-isp
cargo test -p rime-native-gpu
```

Expected: PASS, with native GPU adapter-unavailable behavior following existing skip handling.

**Step 3: Run actual CLI smoke**

```bash
cargo run -p rime-cli -- inspect pipeline/normal/P1020601.dng
cargo run -p rime-cli -- render pipeline/normal/P1020601.dng --output target/wbc-preview.png
```

Expected: metadata inspection succeeds and render completes with finite output. For a Pixel WhiteXY fixture, the same commands must also complete without `missing AsShotNeutral`.

**Step 4: Run workspace verification**

```bash
cargo test --workspace
npm test
npm run typecheck
```

Expected: PASS with no new warnings or generated-contract drift.

**Step 5: Commit verification-only fixes if needed**

Only commit actual source/test adjustments discovered by the checks:

```bash
git add <affected-files>
git commit -m "test: verify metadata white balance pipeline"
```
