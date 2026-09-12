// WBC method 00 — white balance with optional highlight recovery.
//
// Uniform layout (48 B): gains.xyz = green-normalized RGB gains, gains.w =
// hr_gain (1.0 when HR is off), cfa_pattern = CFA phase map, flags bit 0 =
// enable_highlight_recovery.
//
// Highlight recovery — MATLAB reference structure (white_balancing.m::
// highlight_recovery), divided domain throughout:
//
//   1. divided domain   v = raw * gains[c] / hr_gain (MATLAB balanced_bayer
//      after `wbMask / hrGain`). A NEUTRAL highlight is phase-equal there,
//      so survivor channels ARE the reconstruction. hr_gain is divided out
//      here and multiplied back by DRC's drc_gain (design §3.6) — with HR
//      off, hr_gain = 1.0 and both sides are identity.
//   2. dem_unclip(p)    ~= MATLAB dem_unclip: phase-aligned per-site RGB
//      triple via the demosaic00 kernel, two-tier evidence (soft
//      reliability on unclipped neighbors; unweighted mean when every
//      same-channel neighbor is clipped — a clipped sample is a lower
//      bound of the truth, not zero).
//   3. recovery         MATLAB mask1/mask2, domain-correct desat:
//      deep blow (no reliable partner evidence — survivor plateau only
//      encodes gain ratios) -> G = max(R,B): in the divided domain
//      neutral = phase-EQUAL, so max IS the desaturated estimate (mean
//      would mix unequal plateau values into a magenta triple);
//      shallow blow (partners carry real chroma) -> mean(R,B) (mask2).
//   4. feathering       multi-scale mask blending, MATLAB pyrDec/pyrRec
//      structure collapsed into one pass. The per-level mask mix of the
//      Laplacian pyramid (same blur mask imresized to every level, MATLAB
//      imfilter(gauss3) + per-level mix) is algebraically identical to a
//      single full-resolution blend with that mask, because the mix
//      coefficient is level-independent. The catch exposed by v9: a
//      3x3 kernel only buys ~2 px of transition on a 21 MP boundary. The
//      pyramid's real contribution is the effective kernel WIDTH per
//      band: level k reaches ~2^k px. So the collapsed kernel is the
//      widened multi-scale one: a Gaussian with sigma ~= 4 (three
//      [1,2,1]/4 binomial applications at spacings 1/2/4 have the same
//      variance as a 13-tap sigma-4 kernel), i.e. a ~16 px full-width
//      transition. One separable pass, no scratch buffers, no phase
//      hazards (verified in CPU simulation).
//
// One pass, one entry, no intermediate buffers. HR off = plain gains,
// bit-identical to the no-HR render.

struct WhiteBalanceParams {
  gains: vec4<f32>,
  cfa_pattern: vec4<u32>,
  enable_highlight_recovery: u32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: WhiteBalanceParams;

const CLIP_MARGIN: f32 = 0.987;
const SOFT_LO: f32 = 0.9;

fn channel_at(position: vec2<i32>) -> u32 {
  let phase = u32(position.y & 1) * 2u + u32(position.x & 1);
  return params.cfa_pattern[phase];
}

fn raw_at(position: vec2<i32>) -> f32 {
  return textureLoad(input_tex, position, 0).r;
}

// Divided-domain sample (WB applied, hr_gain divided out) — MATLAB
// balanced_bayer. Neutral highlights are phase-equal here; DRC multiplies
// hr_gain back through drc_gain (identity when HR is off).
fn divided_value(position: vec2<i32>) -> f32 {
  let channel = channel_at(position);
  return raw_at(position) * params.gains[channel] / params.gains.w;
}

fn smoothstep_f(e0: f32, e1: f32, x: f32) -> f32 {
  let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
  return t * t * (3.0 - 2.0 * t);
}

// Per-sample reliability: 1 well below the clip knee, tapering to 0 at clip.
fn reliability(raw: f32) -> f32 {
  return 1.0 - smoothstep_f(SOFT_LO, CLIP_MARGIN, raw);
}

// dem_unclip: 3x3 demosaic00 phase-grouped estimate of the per-site RGB
// triple in the divided domain, soft-reliability weighted, no clip.
// Two-tier evidence (MATLAB averages unconditionally — clipped samples
// carry a LOWER BOUND of the true value, not zero).
fn dem_unclip(position: vec2<i32>, extent: vec2<i32>) -> vec3<f32> {
  var sums = vec3<f32>(0.0);
  var weights = vec3<f32>(0.0);
  var plain_sums = vec3<f32>(0.0);
  var counts = vec3<u32>(0u);
  for (var dy = -1; dy <= 1; dy += 1) {
    for (var dx = -1; dx <= 1; dx += 1) {
      let q = position + vec2<i32>(dx, dy);
      if (q.x < 0 || q.y < 0 || q.x >= extent.x || q.y >= extent.y) { continue; }
      let channel = channel_at(q);
      let raw = raw_at(q);
      if (raw <= 0.0) { continue; }
      let value = divided_value(q);
      let w = reliability(raw);
      sums[channel] += value * w;
      weights[channel] += w;
      plain_sums[channel] += value;
      counts[channel] += 1u;
    }
  }
  let weighted = select(vec3<f32>(0.0), sums / weights, weights > vec3<f32>(1e-6));
  let plain = select(vec3<f32>(0.0), plain_sums / vec3<f32>(counts), counts > vec3<u32>(0u));
  return select(plain, weighted, weights > vec3<f32>(1e-6));
}

// Partner reliability mass over the same 3x3 kernel (drives desat).
fn partner_weights(position: vec2<i32>, extent: vec2<i32>) -> vec3<f32> {
  var weights = vec3<f32>(0.0);
  for (var dy = -1; dy <= 1; dy += 1) {
    for (var dx = -1; dx <= 1; dx += 1) {
      let q = position + vec2<i32>(dx, dy);
      if (q.x < 0 || q.y < 0 || q.x >= extent.x || q.y >= extent.y) { continue; }
      let channel = channel_at(q);
      let raw = raw_at(q);
      if (raw <= 0.0) { continue; }
      weights[channel] += reliability(raw);
    }
  }
  return weights;
}

// MATLAB mask1/mask2 recovery, domain-correct desaturation:
//   deep blow (NO reliable partner evidence — the survivor plateau only
//   encodes GAIN RATIOS, not chroma): take the MAX over partners. In the
//   divided domain NEUTRAL = phase-EQUAL, and max equalizes all phases to
//   the plateau upper bound (MATLAB G = max(R,B)); a mean mixes unequal
//   plateau values into a magenta triple (1.61, 1.31, 1.09).
//   shallow blow (reliable partners carry REAL chroma): mean(R,B) (MATLAB
//   mask2), which blends genuine color evidence — a true desat in the
//   color sense.
fn recovered_divided(tex: vec3<f32>, weights: vec3<f32>, channel: u32) -> f32 {
  var mean = 0.0;
  var peak = -1.0;
  var partner_weight = 0.0;
  var partners = 0u;
  for (var k = 0u; k < 3u; k += 1u) {
    if (k == channel) { continue; }
    mean += tex[k];
    peak = max(peak, tex[k]);
    partner_weight += weights[k];
    partners += 1u;
  }
  if (partners == 0u || peak <= 0.0) {
    return tex[channel]; // no partner evidence: own estimate.
  }
  mean = mean / f32(partners);
  // Confidence in REAL chroma: rises with reliable partner mass.
  let chroma_confidence = smoothstep_f(0.05, 0.9, partner_weight / 2.0);
  return mix(peak, mean, chroma_confidence);
}

// Feathering structure (fixes the knife edge): the recovery DELTA is a
// hard step at the gate boundary. Blurring only the mask scales the step
// down but never widens it. Correct form: blur the delta field itself
// (zero outside the gate — continuous extension), then add everywhere:
//   out = value + blur_wide(D)
// Outside pixels near the boundary receive partial headroom (they are
// near-clipped themselves), inside keeps ~full delta — a TWO-SIDED ramp,
// which is exactly what a delta-pyramid blend produces at its coarse
// levels. Sparse 13-tap Gaussian, sigma ~= 4 (~16 px full width).

// Gated recovery delta at a position (0 where not clipped).
fn delta_field(position: vec2<i32>, extent: vec2<i32>) -> f32 {
  if (raw_at(position) < CLIP_MARGIN) { return 0.0; }
  let channel = channel_at(position);
  let value = divided_value(position);
  let tex = dem_unclip(position, extent);
  let weights = partner_weights(position, extent);
  let recovered = max(value, recovered_divided(tex, weights, channel));
  return max(recovered - value, 0.0);
}

// Sparse 13-tap Gaussian (sigma ~= 4), weights sum to 1.
fn blurred_delta(position: vec2<i32>, extent: vec2<i32>) -> f32 {
  var sum = 0.35 * delta_field(position, extent);
  sum += 0.1 * (delta_field(position + vec2<i32>(4, 0), extent)
    + delta_field(position - vec2<i32>(4, 0), extent)
    + delta_field(position + vec2<i32>(0, 4), extent)
    + delta_field(position - vec2<i32>(0, 4), extent));
  sum += 0.05 * (delta_field(position + vec2<i32>(8, 0), extent)
    + delta_field(position - vec2<i32>(8, 0), extent)
    + delta_field(position + vec2<i32>(0, 8), extent)
    + delta_field(position - vec2<i32>(0, 8), extent));
  sum += 0.0125 * (delta_field(position + vec2<i32>(4, 4), extent)
    + delta_field(position - vec2<i32>(4, 4), extent)
    + delta_field(position + vec2<i32>(4, -4), extent)
    + delta_field(position - vec2<i32>(4, -4), extent));
  return sum;
}

// Per-position WBC sample (shared by the compute entry and fused adapters).
fn wbc_sample(position: vec2<i32>, extent: vec2<i32>) -> f32 {
  let value = divided_value(position);
  if (params.enable_highlight_recovery != 0u) {
    // Two-sided feather: blurred gated delta bleeds partial recovery
    // across the boundary; flat regions contribute exactly 0.
    return value + blurred_delta(position, extent);
  }
  return value;
}

@compute @workgroup_size(8, 8)
fn wbc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let position = vec2<i32>(gid.xy);
  // Output port carries Rime.Q s0.14 (container max 1 - 2^-14): clamp the
  // divided-domain value so R sites beyond the container cannot leak past
  // the port. Headroom handoff to DRC happens through the hr_gain scalar,
  // not through out-of-container values.
  let out = min(wbc_sample(position, vec2<i32>(extent)), 1.0 - 1.0 / 16384.0);
  textureStore(output_tex, position, vec4<f32>(out, 0.0, 0.0, 1.0));
}
