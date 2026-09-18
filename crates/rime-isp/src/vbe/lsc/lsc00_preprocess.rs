use super::lsc_common::{
    MESH_HEADER_BYTES, VIGNETTE_MESH_POINTS, mesh_geometry, normalize_mesh_area,
    rasterize_vignette_mesh,
};
use crate::operator::{
    GainMapParameters, ModuleParameterPacket, ModuleParameterResource, OperatorError,
    PreprocessContext,
};

/// Uniform layout (8 `u32` words): `mesh_count`, `cfa_pattern[0..4]`,
/// `width`, `height`.
const UNIFORM_U32S: usize = 8;

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "frame extents are validated by the raw source far below the u32 mantissa limit"
    )]
    let extent = [context.width as f32, context.height as f32];

    // DNG `dng_area_spec::ScaledOverlap` semantics (see
    // `normalize_mesh_area`): an empty spec covers the whole image; a
    // non-empty spec intersects with the frame bounds. The frozen area
    // is a hard pixel mask in `lsc00.wgsl`; the mesh interpolates in
    // whole-image normalized coordinates.
    #[expect(
        clippy::cast_possible_wrap,
        reason = "frame extents are validated by the raw source far below i32 overflow"
    )]
    let frame_area = [0, 0, context.height as i32, context.width as i32];

    let mut meshes: Vec<GainMapParameters> = context
        .gain_maps
        .iter()
        .cloned()
        .map(|mut mesh| {
            mesh.area = normalize_mesh_area(mesh.area, frame_area);
            mesh
        })
        .collect();
    for opcode in &context.vignette_radial {
        let entries = rasterize_vignette_mesh(
            module_id,
            &opcode.coefficients,
            &opcode.optical_center,
            extent,
        )?;
        meshes.push(GainMapParameters {
            points: [VIGNETTE_MESH_POINTS, VIGNETTE_MESH_POINTS],
            spacing: [1.0 / 32.0, 1.0 / 32.0],
            origin: [0.0, 0.0],
            planes: 1,
            area: frame_area,
            row_pitch: 1,
            col_pitch: 1,
            entries,
        });
    }

    let mesh_count = u32::try_from(meshes.len()).map_err(|_| OperatorError::Preprocess {
        module_id,
        reason: "gain mesh count exceeds u32",
    })?;
    let mut uniform = Vec::with_capacity(UNIFORM_U32S * size_of::<u32>());
    uniform.extend_from_slice(&mesh_count.to_ne_bytes());
    uniform.extend_from_slice(&context.cfa_pattern[0].to_ne_bytes());
    uniform.extend_from_slice(&context.cfa_pattern[1].to_ne_bytes());
    uniform.extend_from_slice(&context.cfa_pattern[2].to_ne_bytes());
    uniform.extend_from_slice(&context.cfa_pattern[3].to_ne_bytes());
    uniform.extend_from_slice(&context.width.to_ne_bytes());
    uniform.extend_from_slice(&context.height.to_ne_bytes());
    uniform.resize(UNIFORM_U32S * size_of::<u32>(), 0);

    let (headers, entries_bytes) = freeze_mesh_buffers(&meshes, module_id)?;

    let mut packet = ModuleParameterPacket::new(module_id, method, context.identity, &uniform)?;
    let header_count = u32::try_from(headers.len() / MESH_HEADER_BYTES).map_err(|_| {
        OperatorError::Preprocess {
            module_id,
            reason: "gain mesh header count exceeds u32",
        }
    })?;
    packet.push_resource(ModuleParameterResource::new(
        "gain_mesh_headers",
        [header_count.max(1), 1, 1],
        headers,
    ))?;
    let entry_count = u32::try_from(entries_bytes.len() / size_of::<f32>()).map_err(|_| {
        OperatorError::Preprocess {
            module_id,
            reason: "gain mesh entry count exceeds u32",
        }
    })?;
    packet.push_resource(ModuleParameterResource::new(
        "gain_mesh_entries",
        [entry_count.max(1), 1, 1],
        entries_bytes,
    ))?;
    Ok(packet)
}

fn freeze_mesh_buffers(
    meshes: &[GainMapParameters],
    module_id: &'static str,
) -> Result<(Vec<u8>, Vec<u8>), OperatorError> {
    let mut headers = Vec::with_capacity(meshes.len() * MESH_HEADER_BYTES);
    let mut entries_bytes = Vec::new();
    for mesh in meshes {
        validate_mesh(mesh, module_id)?;
        let geometry = mesh_geometry(module_id, mesh)?;
        let entries_offset =
            u32::try_from(entries_bytes.len() / size_of::<f32>()).map_err(|_| {
                OperatorError::Preprocess {
                    module_id,
                    reason: "gain mesh entries offset exceeds u32",
                }
            })?;
        headers.extend_from_slice(&geometry.points[0].to_ne_bytes());
        headers.extend_from_slice(&geometry.points[1].to_ne_bytes());
        headers.extend_from_slice(&geometry.planes.to_ne_bytes());
        headers.extend_from_slice(&entries_offset.to_ne_bytes());
        headers.extend_from_slice(&geometry.spacing[0].to_ne_bytes());
        headers.extend_from_slice(&geometry.spacing[1].to_ne_bytes());
        headers.extend_from_slice(&geometry.origin[0].to_ne_bytes());
        headers.extend_from_slice(&geometry.origin[1].to_ne_bytes());
        headers.extend_from_slice(&geometry.area[0].to_ne_bytes());
        headers.extend_from_slice(&geometry.area[1].to_ne_bytes());
        headers.extend_from_slice(&geometry.area[2].to_ne_bytes());
        headers.extend_from_slice(&geometry.area[3].to_ne_bytes());
        headers.extend_from_slice(&geometry.row_pitch.to_ne_bytes());
        headers.extend_from_slice(&geometry.col_pitch.to_ne_bytes());
        for entry in &mesh.entries {
            entries_bytes.extend_from_slice(&entry.to_ne_bytes());
        }
    }
    Ok((headers, entries_bytes))
}

/// Validates one mesh's declared geometry before it is frozen into the
/// packet; `mesh_geometry` separately enforces f32 representability.
fn validate_mesh(mesh: &GainMapParameters, module_id: &'static str) -> Result<(), OperatorError> {
    if mesh.points[0] == 0 || mesh.points[1] == 0 || mesh.planes == 0 {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "gain map mesh must declare at least one point and plane",
        });
    }
    let expected = usize::try_from(mesh.points[0])
        .ok()
        .and_then(|rows| {
            usize::try_from(mesh.planes)
                .ok()
                .map(|planes| rows * planes)
        })
        .and_then(|rows_planes| {
            usize::try_from(mesh.points[1])
                .ok()
                .map(|cols| rows_planes * cols)
        })
        .unwrap_or(usize::MAX);
    if mesh.entries.len() != expected {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "gain map mesh entry count does not match points and planes",
        });
    }
    if !mesh
        .spacing
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
        || !mesh.origin.iter().all(|value| value.is_finite())
    {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "gain map mesh spacing must be positive and geometry finite",
        });
    }
    if !mesh.entries.iter().all(|entry| entry.is_finite()) {
        return Err(OperatorError::Preprocess {
            module_id,
            reason: "gain map mesh entries must be finite",
        });
    }
    Ok(())
}
