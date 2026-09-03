<!-- markdownlint-disable MD013 -->

# DNG AsShotWhiteXY White-Balance Design

## Goal

Support DNG files, including Google Pixel DNGs, that provide `AsShotWhiteXY` without `AsShotNeutral`. Compute white-balance gains during preprocess and expose only normalized RGB gains to the WBC shader.

## Current Problem

The project currently exposes `as_shot_neutral: [f64; 3]` as if the field were always present. The `gamut-dng 1.0.0` decoder also requires `AsShotNeutral` while reconstructing `CameraProfile`, so a DNG containing only `AsShotWhiteXY` fails before project-owned metadata and preprocessing run. The WBC WGSL shader declares gain-like parameters in the operator manifest but currently applies hard-coded red and blue gains.

## Design

### Metadata contract

Preserve source metadata rather than replacing one DNG field with another:

- `as_shot_neutral: Option<[f64; 3]>`
- `as_shot_white_xy: Option<[f64; 2]>`
- `color_matrix1: [f64; 9]`
- `color_matrix2: Option<[f64; 9]>`

The inspector and serialized descriptor expose both optional fields. `AsShotNeutral` remains the preferred source when both fields exist.

### White-balance source precedence

Preprocess resolves one neutral vector using this order:

1. `AsShotNeutral`.
2. `AsShotWhiteXY` converted through `ColorMatrix2`.
3. `AsShotWhiteXY` converted through `ColorMatrix1`.
4. Stable missing/invalid-calibration error.

`ColorMatrix2` is preferred for the `AsShotWhiteXY` fallback, matching the reference MATLAB implementation. `ColorMatrix1` is the compatibility fallback when the second matrix is absent.

### AsShotWhiteXY conversion

For `AsShotWhiteXY = [x, y]`, assume `Y = 1` and calculate:

```text
X = x / y
Z = (1 - x - y) / y
camera_rgb = ColorMatrix * [X, Y, Z]
neutral = camera_rgb / camera_rgb.g
```

The input coordinates, matrix entries, and resulting neutral components must be finite. `y` must be positive, `x` and `y` must represent a valid chromaticity (`x > 0`, `y > 0`, `x + y < 1`), and all resulting neutral components must be strictly positive.

### Preprocess API

Add a reusable project-owned preprocessing function that accepts DNG metadata and returns:

```rust
pub struct WhiteBalanceGains {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}
```

It first resolves the neutral vector according to the precedence above, computes reciprocal channel gains, and normalizes the gains by green:

```text
raw_gains = 1 / neutral
rgb_gains = raw_gains / raw_gains.g
```

Both the regular WBC path and the native fused GPU path call this preprocessing logic. The DNG reader retains raw metadata semantics; it does not expose computed gains as a replacement for the source tags.

### Decoder integration

Because `gamut-dng 1.0.0` rejects missing `AsShotNeutral` inside its private profile decoder, the DNG boundary must make the profile decodable without losing the original field information. The implementation should use the smallest project-local adapter or dependency patch that allows raw decoding with an `AsShotWhiteXY`-only file. The adapter must preserve the original optional metadata values and must not silently invent `AsShotNeutral` in the inspector contract.

If the dependency cannot expose enough IFD information for a clean adapter, make the dependency change explicit and local to the DNG boundary rather than embedding tag parsing in the WBC or GPU layers.

### WBC shader contract

The WBC operator continues to expose exactly these parameters:

```text
red_gain green_gain blue_gain
```

The WGSL implementation selects the gain by CFA channel and multiplies the input sample. No hard-coded channel gains remain. The native fused shader uses the same three preprocessed gains and the same CFA mapping.

## Error handling

Return stable project errors for:

- missing both white-balance tags;
- invalid/non-finite `AsShotNeutral`;
- invalid/non-finite `AsShotWhiteXY`;
- absent or invalid color matrix for a WhiteXY fallback;
- non-positive converted camera RGB values;
- non-finite computed gains.

Do not silently fall back to fixed gains or identity gains.

## Verification

Add behavior-focused tests for:

- `AsShotNeutral` taking precedence over `AsShotWhiteXY`;
- WhiteXY conversion through `ColorMatrix2` matching the MATLAB formula;
- fallback to `ColorMatrix1` when `ColorMatrix2` is absent;
- invalid coordinate/matrix/result rejection;
- serialized metadata retaining optional source fields;
- WBC WGSL consuming uniform RGB gains rather than hard-coded values;
- native fused rendering using metadata-derived gains;
- existing GH5S local fixture regression.

Local DNG tests use fixtures under `C:\Users\zisheng\Documents\cao\99_data\isp\pana_gh5s`; no fixture is committed.
