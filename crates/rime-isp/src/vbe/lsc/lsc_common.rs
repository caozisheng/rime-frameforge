//! Stateless helpers shared by the LSC stage: `FixVignetteRadial` → mesh
//! rasterization and the CPU reference of the WGSL mesh interpolation.
//! No manifests, method selection, or cross-frame state here (AGENTS.md).
//!
//! Interpolation contract (shared with `lsc00.wgsl`, DNG SDK semantics):
//! a pixel at integer index `id` samples the mesh at the normalized
//! pixel-center coordinate `u = (id + 0.5) / extent`; the mesh position is
//! `pos = (u - origin) / spacing`, clamped to the node grid, bilinearly
//! interpolated over row-major `[row][col][plane]` entries with
//! `plane = min(channel, planes - 1)`.

use crate::operator::{GainMapParameters, OperatorError};

/// Side length of the square mesh a `FixVignetteRadial` polynomial is
/// rasterized onto: 33 nodes → exact node hits at every 1/32 position.
pub const VIGNETTE_MESH_POINTS: u32 = 33;

/// Mesh header record size as frozen in the LSC packet (bytes).
pub const MESH_HEADER_BYTES: usize = 56;

/// The geometry half of one frozen mesh header (no entries offset).
#[derive(Clone, Copy)]
pub struct MeshGeometry {
    /// Node counts `[vertical, horizontal]` (at least 1 each).
    pub points: [u32; 2],
    /// Grid spacing in normalized pixel coordinates.
    pub spacing: [f32; 2],
    /// Grid origin in normalized pixel coordinates.
    pub origin: [f32; 2],
    /// Interleaved plane count per node (at least 1).
    pub planes: u32,
    /// Application bounds `[top, left, bottom, right]` in pixels,
    /// exclusive at bottom/right. Mesh interpolation always runs over
    /// the whole image; pixels outside the bounds receive identity —
    /// DNG `dng_area_spec` semantics.
    pub area: [i32; 4],
    /// Application grid pitch in pixels (at least 1): the gain applies
    /// only where the pixel sits on the pitch lattice anchored at the
    /// area's top/left corner (DNG checkerboard semantics).
    pub row_pitch: u32,
    pub col_pitch: u32,
}

/// Evaluates the `FixVignetteRadial` gain at a pixel center — an `f32` port
/// of the WGSL formula: elliptical radius normalized by the farthest
/// corner, Horner-form polynomial `1 + r2 * (k0 + r2 * (...))`.
#[must_use]
pub fn vignette_gain(
    coefficients: &[f64; 5],
    optical_center: &[f64; 2],
    pixel: [f32; 2],
    extent: [f32; 2],
) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "rasterize_vignette_mesh validated the f64 inputs are f32-representable"
    )]
    let center = [optical_center[0] as f32, optical_center[1] as f32];
    let center = [extent[0] * center[0], extent[1] * center[1]];
    let delta = [pixel[0] - center[0], pixel[1] - center[1]];
    let farthest = [
        center[0].max(extent[0] - center[0]),
        center[1].max(extent[1] - center[1]),
    ];
    let maximum_radius_squared =
        (farthest[0] * farthest[0] + farthest[1] * farthest[1]).max(1.0e-12);
    let radius_squared = (delta[0] * delta[0] + delta[1] * delta[1]) / maximum_radius_squared;
    let mut gain = gpu_k(coefficients[4]);
    gain = gpu_k(coefficients[3]) + radius_squared * gain;
    gain = gpu_k(coefficients[2]) + radius_squared * gain;
    gain = gpu_k(coefficients[1]) + radius_squared * gain;
    1.0 + radius_squared * (gpu_k(coefficients[0]) + radius_squared * gain)
}

/// Converts one validated coefficient to its `f32` GPU value.
#[expect(
    clippy::cast_possible_truncation,
    reason = "rasterize_vignette_mesh validated the f64 inputs are f32-representable"
)]
fn gpu_k(value: f64) -> f32 {
    value as f32
}

/// Rasterizes one `FixVignetteRadial` polynomial into a single-plane
/// `33 × 33` mesh. Node `(row, col)` holds the gain at the pixel center
/// whose normalized coordinate hits the node exactly:
/// `(id + 0.5) / extent = (col / 32, row / 32)`.
///
/// # Errors
///
/// Returns an `OperatorError::Preprocess` when a coefficient or center is
/// non-finite, not `f32`-representable, or a center lies outside `[0, 1]`.
pub fn rasterize_vignette_mesh(
    module_id: &'static str,
    coefficients: &[f64; 5],
    optical_center: &[f64; 2],
    extent: [f32; 2],
) -> Result<Vec<f32>, OperatorError> {
    let invalid = |reason: &'static str| OperatorError::Preprocess { module_id, reason };
    for value in coefficients.iter().chain(optical_center) {
        if !value.is_finite() || value.abs() > f64::from(f32::MAX) {
            return Err(invalid(
                "FixVignetteRadial values must be finite, GPU-representable, and not overflow f32",
            ));
        }
    }
    if optical_center
        .iter()
        .any(|value| !(0.0..=1.0).contains(value))
    {
        return Err(invalid(
            "FixVignetteRadial optical center must be within [0, 1]",
        ));
    }

    let divisions = f64::from(VIGNETTE_MESH_POINTS - 1);
    let side = usize::try_from(VIGNETTE_MESH_POINTS).expect("u32 fits usize");
    let mut entries = Vec::with_capacity(side * side);
    for row in 0..VIGNETTE_MESH_POINTS {
        for col in 0..VIGNETTE_MESH_POINTS {
            // Mesh interpolation samples pixel centers: u = (id + 0.5) / extent.
            // Node (row, col) must hold the gain at the center whose
            // normalized coordinate hits the node exactly:
            // id = node_pos * extent - 0.5.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "node positions stay within the extent, which is f32 already"
            )]
            let node_x = ((f64::from(col) / divisions) * f64::from(extent[0])) as f32;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "node positions stay within the extent, which is f32 already"
            )]
            let node_y = ((f64::from(row) / divisions) * f64::from(extent[1])) as f32;
            entries.push(vignette_gain(
                coefficients,
                optical_center,
                [node_x, node_y],
                extent,
            ));
        }
    }
    Ok(entries)
}

/// Reference bilinear mesh gain — numerically identical to `lsc00.wgsl`.
/// `channel` is the CFA phase channel; `pixel_index` is the integer
/// pixel coordinate (`id`); `entries_offset` is in `f32` units. Pixels
/// outside the mesh's application area return identity (`1.0`).
///
/// # Panics
///
/// Panics when an interpolated mesh entry index exceeds `usize` — the
/// caller-frozen packet geometry keeps every index far below that bound.
#[must_use]
pub fn mesh_gain(
    geometry: MeshGeometry,
    entries: &[f32],
    entries_offset: usize,
    channel: u32,
    pixel_index: [u32; 2],
    extent: [f32; 2],
) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "pixel indices stay far below the u32 mantissa limit for real frames"
    )]
    let pixel = [pixel_index[0] as f32 + 0.5, pixel_index[1] as f32 + 0.5];
    #[expect(
        clippy::cast_possible_wrap,
        reason = "pixel indices and area bounds share the validated sub-i32 frame range"
    )]
    let index = [pixel_index[0] as i32, pixel_index[1] as i32];
    #[expect(
        clippy::cast_sign_loss,
        reason = "area bounds are validated to sit inside the frame; the subtraction cannot go negative here"
    )]
    let lattice = [
        (index[0] - geometry.area[1]) as u32 % geometry.col_pitch,
        (index[1] - geometry.area[0]) as u32 % geometry.row_pitch,
    ];
    let inside = index[0] >= geometry.area[1]
        && index[0] < geometry.area[3]
        && index[1] >= geometry.area[0]
        && index[1] < geometry.area[2]
        && lattice[0] == 0
        && lattice[1] == 0;
    if !inside {
        return 1.0;
    }
    let column_position = (pixel[0] / extent[0] - geometry.origin[1]) / geometry.spacing[1];
    let row_position = (pixel[1] / extent[1] - geometry.origin[0]) / geometry.spacing[0];
    let plane = channel.min(geometry.planes.saturating_sub(1));
    #[expect(
        clippy::cast_precision_loss,
        reason = "node counts are small mesh dimensions far below 2^24"
    )]
    let last_column = geometry.points[1] as f32 - 1.0;
    #[expect(
        clippy::cast_precision_loss,
        reason = "node counts are small mesh dimensions far below 2^24"
    )]
    let last_row = geometry.points[0] as f32 - 1.0;
    let column = column_position.clamp(0.0, last_column);
    let row = row_position.clamp(0.0, last_row);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "position is clamped to [0, last_node] before the cast"
    )]
    let col0 = column as u32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "position is clamped to [0, last_node] before the cast"
    )]
    let row0 = row as u32;
    let col1 = (col0 + 1).min(geometry.points[1] - 1);
    let row1 = (row0 + 1).min(geometry.points[0] - 1);
    #[expect(
        clippy::cast_precision_loss,
        reason = "node indices are small mesh dimensions far below 2^24"
    )]
    let fx = column - col0 as f32;
    #[expect(
        clippy::cast_precision_loss,
        reason = "node indices are small mesh dimensions far below 2^24"
    )]
    let fy = row - row0 as f32;

    let entry = |row_index: u32, col_index: u32| {
        entries[entries_offset
            + usize::try_from(
                row_index * geometry.points[1] * geometry.planes
                    + col_index * geometry.planes
                    + plane,
            )
            .expect("mesh entry index fits usize")]
    };
    let top = entry(row0, col0) + (entry(row0, col1) - entry(row0, col0)) * fx;
    let bottom = entry(row1, col0) + (entry(row1, col1) - entry(row1, col0)) * fx;
    top + (bottom - top) * fy
}

/// Reads one `GainMapParameters` mesh's geometry as the packet header
/// fields, validating that every `f64` survives the `f32` conversion.
///
/// # Errors
///
/// Returns an `OperatorError::Preprocess` with a stable reason when the
/// mesh geometry is not GPU-representable.
pub fn mesh_geometry(
    module_id: &'static str,
    mesh: &GainMapParameters,
) -> Result<MeshGeometry, OperatorError> {
    let invalid = |reason: &'static str| OperatorError::Preprocess { module_id, reason };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "values are validated as finite and within the f32 range before encoding"
    )]
    let f32_value = |value: f64| value as f32;
    if mesh.points[0] == 0 || mesh.points[1] == 0 || mesh.planes == 0 {
        return Err(invalid(
            "gain map mesh must have at least one node and plane",
        ));
    }
    if !mesh
        .spacing
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
        || !mesh.origin.iter().all(|value| value.is_finite())
        || mesh
            .spacing
            .iter()
            .any(|value| value.abs() > f64::from(f32::MAX))
        || mesh
            .origin
            .iter()
            .any(|value| value.abs() > f64::from(f32::MAX))
    {
        return Err(invalid(
            "gain map mesh geometry must be finite and f32-representable",
        ));
    }
    if mesh.row_pitch == 0 || mesh.col_pitch == 0 {
        return Err(invalid("gain map mesh pitch must be at least 1"));
    }
    if mesh.entries.len()
        != usize::try_from(mesh.points[0] * mesh.points[1] * mesh.planes)
            .expect("mesh dimensions were validated to be small")
        || mesh.entries.iter().any(|entry| !entry.is_finite())
    {
        return Err(invalid(
            "gain map mesh entries must be finite and match the declared dimensions",
        ));
    }
    Ok(MeshGeometry {
        points: mesh.points,
        spacing: [f32_value(mesh.spacing[0]), f32_value(mesh.spacing[1])],
        origin: [f32_value(mesh.origin[0]), f32_value(mesh.origin[1])],
        planes: mesh.planes,
        area: mesh.area,
        row_pitch: mesh.row_pitch,
        col_pitch: mesh.col_pitch,
    })
}

/// Normalizes a DNG `dng_area_spec` `[top, left, bottom, right]` onto the
/// LSC packet's hard pixel mask, following the SDK's `ScaledOverlap`
/// semantics: an empty spec (`bottom <= top || right <= left`, including
/// all-zero) covers the whole image; a non-empty spec intersects with the
/// frame bounds (`fArea & tile`). The mesh itself always interpolates in
/// whole-image normalized coordinates (`dng_gain_map_interpolator` uses
/// `imageBounds`), so the area only gates which pixels receive the gain.
#[must_use]
pub fn normalize_mesh_area(area: [i32; 4], frame_area: [i32; 4]) -> [i32; 4] {
    let [top, left, bottom, right] = area;
    if bottom <= top || right <= left {
        return frame_area;
    }
    [
        top.max(frame_area[0]),
        left.max(frame_area[1]),
        bottom.min(frame_area[2]),
        right.min(frame_area[3]),
    ]
}
