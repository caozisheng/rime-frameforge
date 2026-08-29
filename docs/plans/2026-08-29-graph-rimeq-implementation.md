# Graph Rime.Q Configuration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add graph-level and module-level Rime.Q configuration for the current and future graph pattern, rewrite `rime-quant` around `uX.Y`/`sX.Y` and `ClipType`, and expose the configuration in the tree-based Node inspector when no node is selected.

**Architecture:** Rust owns the serializable graph quantization contract, defaults, profile parsing, and effective-state precedence. The existing Rust asset generator emits the graph presentation/configuration consumed by TypeScript. React owns only the current editable projection and renders it through `InspectorTree`; Worker/WASM commands carry validated configuration snapshots to the runtime authority. Normal Graph remains presentation-only until its execution engine has real output quantization points; no UI state may claim GPU quantization when no executor path exists.

**Tech Stack:** Rust 1.93, Cargo workspace, serde/serde_json, wasm-bindgen, TypeScript 5.9, React 19, Vitest, WebGPU worker, generated TypeScript assets.

---

### Task 1: Define the new Rime.Q value model with failing tests

**Files:**
- Modify: `crates/rime-quant/tests/quantization.rs`
- Modify: `crates/rime-quant/src/profile.rs`
- Modify: `crates/rime-quant/src/quantize.rs`
- Modify: `crates/rime-quant/src/lib.rs`

**Step 1: Write the failing tests**

Add behavior tests for:

- parsing `u0.14` and `s1.12` into integer bits, fractional bits, and signedness;
- formatting profiles back to exactly `uX.Y`/`sX.Y`;
- rejecting missing sign, missing dot, negative bits, non-numeric bits, and `int_bits + frac_bits > 24`;
- unsigned range `[0, 2^X - 2^-Y]` and signed range `[-2^X, 2^X - 2^-Y]`;
- LSB equals `2^-Y`;
- `ClipType::Truncate`, `ClipType::Round`, and `ClipType::Dither` behavior, including the existing floor-plus-half negative-value rule;
- dither only changing the profile LSB and zero input remaining zero;
- non-finite values and unsupported saturation returning explicit errors.

Use the real public API, not implementation helpers or mocks. Keep existing deterministic LFSR/rnd4b assertions as regression coverage.

**Step 2: Run the focused tests to verify RED**

Run: `cargo test -p rime-quant --test quantization`

Expected: FAIL because the new profile parser, formatter, and `ClipType` API do not exist or the expected semantics are not implemented.

**Step 3: Implement the minimal public model**

Replace the old public rounding-centered model with a single contract:

- Add `RimeQProfile` (or the chosen equivalent) with `parse`, `Display`/formatting, `validate`, `lsb`, `qmin`, and `qmax`.
- Add `ClipType` with only the supported truncate, round, and dither policies.
- Keep deterministic dither stream/seed/key data as a separate reusable configuration.
- Make profile validation enforce exact FP32 grid precision and valid dither configuration.
- Make quantization execute `ClipType transform -> fixed-point code -> clamp -> f32 grid value`.
- Remove obsolete public aliases/enums rather than retaining a second contradictory API. Update WGSL export names if needed.

**Step 4: Run the focused tests to verify GREEN**

Run: `cargo test -p rime-quant --test quantization`

Expected: PASS with the new notation, bounds, ClipType, dither, and error behavior.

**Step 5: Commit**

```bash
git add crates/rime-quant
git commit -m "feat: rewrite rime quantization contract"
```

---

### Task 2: Add shared graph quantization contract and effective-state rules

**Files:**
- Modify: `crates/rime-core/src/graph_presentation.rs`
- Modify: `crates/rime-core/src/lib.rs`
- Modify: `crates/rime-core/tests/graph_presentation.rs`
- Possibly modify: `crates/rime-core/src/manifest.rs` if output profile metadata belongs on `PortSpec`

**Step 1: Write failing contract tests**

Add tests for:

- graph config containing a root `enabled` switch and module records;
- module mode being derived from graph presentation rather than writable config;
- global off forcing effective output quantization and dither off;
- disabled and bypass modes forcing effective output quantization and dither off;
- reopening the global switch restoring saved enabled-module preferences;
- unknown module IDs and malformed profiles being rejected;
- only output profiles being represented, with input profile inherited from the upstream output;
- default profiles for BLC, WBC, DEM, and RGB2YUV being `u0.14`, `u0.12`, `u0.12`, and `u0.10`.

**Step 2: Run focused tests to verify RED**

Run: `cargo test -p rime-core --test graph_presentation`

Expected: FAIL because the graph quantization contract and defaults do not exist.

**Step 3: Implement the contract and precedence**

Add serializable Rust types for graph config, module preference, effective module state, and profile/ClipType references. Add a graph-level constructor/default builder that walks the existing `GraphPresentation` tree and creates records for executable output modules. Implement one authoritative resolver with this precedence:

```text
!graph.enabled                       => effective output=false, dither=false
mode disabled or bypass             => effective output=false, dither=false
otherwise                            => saved module output/dither preferences
```

Preserve saved preferences while reporting effective state separately. Expose the types from `rime-core`. Do not add enable/bypass setters; those are presentation-derived and read-only.

**Step 4: Run focused tests to verify GREEN**

Run: `cargo test -p rime-core --test graph_presentation`

Expected: PASS, including the four requested defaults and precedence tests.

**Step 5: Commit**

```bash
git add crates/rime-core
 git commit -m "feat: add graph quantization contract"
```

---

### Task 3: Attach output profiles to ISP definitions and generated graph assets

**Files:**
- Modify: `crates/rime-isp/src/operator.rs`
- Modify: `crates/rime-isp/src/graph.rs`
- Modify: `crates/rime-isp/src/generated.rs`
- Modify: `crates/rime-isp/src/vfe/blc/mod.rs`
- Modify: `crates/rime-isp/src/vbe/white_balance/mod.rs`
- Modify: `crates/rime-isp/src/vbe/dem/mod.rs`
- Modify: `crates/rime-isp/src/vbe/rgb_to_yuv/mod.rs`
- Modify: `crates/rime-isp/tests/normal_manifest.rs`
- Modify: `crates/rime-isp/tests/operator_methods.rs` if its fixtures assert complete operator definitions
- Regenerate: `web/src/generated/normal_graph.generated.ts`
- Regenerate: `web/src/generated/normal_manifest.generated.ts`

**Step 1: Write failing generation/manifest tests**

Add assertions that generated Normal Graph/manifest output exposes output Rime.Q defaults for the four named modules and that input quantization is inherited rather than independently configured. Assert generated data remains deterministic.

**Step 2: Run focused tests to verify RED**

Run: `cargo test -p rime-isp --test normal_manifest`

Expected: FAIL because operator and generated-port metadata do not contain output profiles.

**Step 3: Implement metadata and generator output**

Extend the operator/port metadata with output Rime.Q profile and any sensor effective-bit fact needed by the inspector. Set the requested defaults exactly. Ensure BLC’s generated output uses unsigned `u0.14`; do not retain the old signed normalized-RAW implication in generated metadata. Update the generator to serialize graph quantization defaults from Rust, not a hand-written TypeScript module list. Run the repository generation command after Rust changes.

**Step 4: Run focused tests and regenerate assets**

Run: `cargo test -p rime-isp --test normal_manifest`

Then run: `npm run generate:manifest`

Expected: Cargo tests PASS and generated TypeScript assets contain the same four profiles and graph hierarchy.

**Step 5: Commit**

```bash
git add crates/rime-isp web/src/generated
 git commit -m "feat: generate graph rime.q defaults"
```

---

### Task 4: Carry graph configuration through web contracts, WASM authority, and Worker

**Files:**
- Modify: `web/src/contracts.ts`
- Modify: `apps/desktop/src/runtime/worker-bridge.ts`
- Modify: `apps/desktop/src/runtime/wasm-runtime.ts`
- Modify: `crates/rime-wasm/src/lib.rs`
- Modify: `apps/desktop/src/pipeline.worker.ts`
- Modify: `web/src/runtime-controller.ts` only if execution identity/config revision needs a new field
- Add/modify: relevant Rust WASM/runtime tests under `crates/rime-wasm` or `crates/rime-core/tests`

**Step 1: Write failing command/authority tests**

Add tests for:

- serializing a graph quantization snapshot through the runtime command contract;
- accepting graph-level and module-level updates only while the lifecycle is stopped or completed;
- incrementing the numeric/config revision on accepted updates;
- rejecting malformed/unknown module configuration through the authority;
- preserving module preference values when global quantization is turned off.

**Step 2: Run focused tests to verify RED**

Run the smallest relevant Rust and TypeScript tests, for example:

- `cargo test -p rime-core`
- `npm test -- apps/desktop/tests/node-inspector.test.tsx`

Expected: FAIL because command variants, authority methods, and revision behavior are absent.

**Step 3: Implement command and authority wiring**

Add typed `set_graph_quantization` and `set_module_quantization` commands. Add matching WorkerBridge methods and queue handling. Keep WASM/runtime as the lifecycle/config authority; map its snapshot into the existing envelope without allowing UI-only state to bypass validation. Recreate or update the future quantization execution plan boundary when configuration changes. Since Normal is currently presentation-only, keep executor behavior explicit: configuration is accepted and surfaced, but no fake GPU quantization is reported until an output QuantPoint exists.

**Step 4: Run focused tests to verify GREEN**

Run the focused Rust and TypeScript tests again. Expected: PASS with command validation, lifecycle gating, and revision behavior covered.

**Step 5: Commit**

```bash
git add web/src/contracts.ts apps/desktop/src/runtime crates/rime-wasm/src crates/rime-core
 git commit -m "feat: wire graph quantization commands"
```

---

### Task 5: Add graph-empty selection and graph-level Inspector tree

**Files:**
- Modify: `apps/desktop/src/App.tsx`
- Modify: `apps/desktop/src/components/NormalGraphCanvas.tsx`
- Modify: `apps/desktop/src/components/NodeInspector.tsx`
- Modify: `apps/desktop/src/components/InspectorTree.tsx` only if control rendering needs an accessible switch/slider primitive
- Add/modify: `apps/desktop/tests/node-inspector.test.tsx`
- Add/modify: `apps/desktop/tests/app-layout.test.tsx`
- Add/modify: a focused graph canvas test if pane-click behavior has an existing test seam
- Modify: `apps/desktop/src/styles.css` for switch/slider and disabled-state presentation

**Step 1: Write failing React tests**

Add tests that render `NodeInspector` with `nodeId={null}` and assert:

- graph title/configuration is shown;
- tree role is present;
- VFE, VBE, VPE, pass-1, pass-2, pass-3 and module labels are present;
- the top-level Rime.Q slider exists and is accessible by label;
- module mode and bypass are read-only values;
- output profile, output switch, dither switch, and ClipType controls appear for module leaves;
- global off disables effective module controls while preserving the saved preference on reopen;
- disabled/bypass modules have quantization/dither controls disabled;
- BLC/WBC/DEM/RGB2YUV defaults render correctly.

Add a canvas test or interaction test asserting pane click clears selection and node click selects the node.

**Step 2: Run focused tests to verify RED**

Run: `npm test -- apps/desktop/tests/node-inspector.test.tsx apps/desktop/tests/app-layout.test.tsx`

Expected: FAIL because `nodeId` is currently required, the graph tree branch does not exist, and pane selection cannot be cleared.

**Step 3: Implement graph inspector rendering and state flow**

- Change selection state to `string | null` and initialize the current graph with no node selected.
- Add `onPaneClick` to clear selection; keep node click selecting the concrete execution node/group ID.
- Add a graph configuration state initialized from generated Rust defaults.
- Pass the config and callbacks into `NodeInspector`.
- Build the graph tree recursively from `normalGraphPresentation.nodes` and `parent_id`; do not duplicate the module hierarchy in React.
- Render the graph root controls and module controls through `InspectorTree` control slots.
- Use semantic labels, native checkbox/range/select controls, and `disabled` attributes for read-only or forced-off states.
- Compute effective state through the shared contract helper or generated equivalent, not an independent precedence implementation.
- Dispatch accepted edits through WorkerBridge and update local state only in the same validated state transition path.
- Keep node inspector behavior unchanged for selected nodes and preserve the DNG special case.

**Step 4: Run focused tests to verify GREEN**

Run: `npm test -- apps/desktop/tests/node-inspector.test.tsx apps/desktop/tests/app-layout.test.tsx`

Expected: PASS with graph tree, global/module controls, selection clearing, and existing node inspector behavior.

**Step 5: Commit**

```bash
git add apps/desktop/src apps/desktop/tests
 git commit -m "feat: add graph rime.q inspector"
```

---

### Task 6: Rewrite the quantization design document around the concise contract

**Files:**
- Rewrite: `docs/rime-frameforge-quantization-design.md`

**Step 1: Write the final document structure**

Replace contradictory legacy sections with concise sections covering:

1. `uX.Y`/`sX.Y` notation, sign-bit meaning, dynamic range, precision, LSB, and FP32 carrier;
2. graph/global/module controls and effective-state precedence;
3. output-only quantization and input inheritance from the previous module;
4. `ClipType` semantics and saturation order;
5. defaults BLC `u0.14`, WBC `u0.12`, DEM `u0.12`, RGB2YUV `u0.10`;
6. 10-bit sensor to `u0.14`: four invalid low fractional bits and dither for banding/patching/oil-paint artifacts;
7. 14-bit sensor to `u0.14`: all fractional bits valid, gamma/scale lift, and high-bit precision tradeoff;
8. gain/headroom from `u0.y` to `u1.y`/`u2.y`, clipping versus physical signal preservation, and dynamic-range compression to `u0.(y+1)`;
9. the rule that dither treats invalid quantization artifacts while compression/tone/gamma preserves valid signal;
10. implementation and verification constraints for Rust/WGSL consistency.

Ensure the document explicitly states that profile precision cannot create sensor information and that BLC uses unsigned `u0.14` per the new requirement.

**Step 2: Validate the rewritten document**

Run: `npx --yes markdownlint-cli2 docs/rime-frameforge-quantization-design.md`

Expected: 0 issues.

**Step 3: Commit**

```bash
git add docs/rime-frameforge-quantization-design.md
 git commit -m "docs: simplify rime.q physical design"
```

---

### Task 7: Run contract-level, build, and actual UI verification

**Files:**
- No source changes unless verification exposes a defect.

**Step 1: Run Rust verification**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: all commands pass with no formatting, test, or lint failures.

**Step 2: Run TypeScript and web verification**

Run:

```bash
npm test
npm run typecheck
npm run build
```

Expected: Vitest, TypeScript, WASM generation, and Vite build pass.

**Step 3: Exercise the actual desktop web surface**

Launch the existing Vite/Tauri-compatible desktop surface using the repository’s normal dev command. In the browser:

1. select the current graph;
2. click graph whitespace to ensure no module is selected;
3. verify the graph tree shows overall, VFE/VBE/VPE, pass-1/2/3 and module hierarchy;
4. toggle overall Rime.Q off and confirm module effective controls become disabled;
5. toggle it back on and confirm enabled-module preferences return;
6. expand a disabled or bypass module and confirm output quantization/dither remain disabled;
7. expand BLC/WBC/DEM/RGB2YUV and confirm the four default profiles;
8. select a module and verify the existing node inspector still works.

Expected: visual and interaction behavior matches the approved design. Report any unavailable visual runtime explicitly rather than substituting static markup checks.

**Step 4: Commit any verification fixes**

If verification reveals defects, add a regression test first, apply the minimal fix, rerun the affected check, then commit with a focused message. Do not suppress warnings or broaden scope.
