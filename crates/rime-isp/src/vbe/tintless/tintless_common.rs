#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::large_stack_arrays,
    clippy::needless_range_loop,
    reason = "fixed LCST/tintless grids and bounded numerical solves use explicit indexed arrays"
)]

use thiserror::Error;

use crate::{
    FrameIdentity, LCST_AVERAGE_CHANNELS, LCST_AVERAGE_GRID_HEIGHT, LCST_AVERAGE_GRID_WIDTH,
    LcstStatisticsPacket, valid_cfa_pattern,
};

pub const TINTLESS_MESH_WIDTH: usize = 65;
pub const TINTLESS_MESH_HEIGHT: usize = 49;
pub const TINTLESS_MESH_CHANNELS: usize = 2;
pub const TINTLESS_MESH_VALUES: usize =
    TINTLESS_MESH_WIDTH * TINTLESS_MESH_HEIGHT * TINTLESS_MESH_CHANNELS;
const CELL_COUNT: usize = LCST_AVERAGE_GRID_WIDTH * LCST_AVERAGE_GRID_HEIGHT;
const RADIAL_KNOTS: usize = 9;
const IRLS_ITERATIONS: usize = 8;
const HUE_LINK_THRESHOLD: f32 = 0.04;
const MINIMUM_COMPONENT_CELLS: usize = 8;
const MINIMUM_RADIAL_SPAN: f32 = 0.08;
const RATIO_EPSILON: f32 = 1.0 / 65_536.0;
const MINIMUM_GREEN: f32 = 1.0 / 1_024.0;
const SATURATION_LIMIT: f32 = 0.98;
const HUBER_DELTA: f32 = 0.03;
const SMOOTHNESS_LAMBDA: f32 = 0.25;
const GAIN_MIN: f32 = 0.5;
const GAIN_MAX: f32 = 2.0;

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TintlessError {
    #[error("tintless LCST producer/consumer frame identity is incompatible")]
    IdentityMismatch,
    #[error("tintless LCST source extent does not match the Bayer input")]
    ExtentMismatch,
    #[error("tintless LCST CFA pattern does not match the Bayer input")]
    CfaMismatch,
    #[error("tintless LCST average contains a non-finite value")]
    NonFiniteAverage,
    #[error("tintless LCST field has insufficient qualified hue components")]
    InsufficientComponents,
    #[error("tintless radial solve is singular")]
    SingularSolve,
    #[error("tintless radial solve produced a non-finite value")]
    NonFiniteSolve,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TintlessGainMesh {
    producer_identity: FrameIdentity,
    consumer_identity: FrameIdentity,
    source_extent: [u32; 2],
    cfa_pattern: [u32; 4],
    entries: Box<[f32]>,
    valid_cells: u32,
    qualified_components: u32,
    residual_rms: [f32; 2],
    clamp_count: u32,
}

impl TintlessGainMesh {
    #[must_use]
    pub fn identity(
        producer_identity: FrameIdentity,
        consumer_identity: FrameIdentity,
        source_extent: [u32; 2],
        cfa_pattern: [u32; 4],
    ) -> Self {
        Self {
            producer_identity,
            consumer_identity,
            source_extent,
            cfa_pattern,
            entries: vec![1.0; TINTLESS_MESH_VALUES].into_boxed_slice(),
            valid_cells: 0,
            qualified_components: 0,
            residual_rms: [0.0; 2],
            clamp_count: 0,
        }
    }

    #[must_use]
    pub fn producer_identity(&self) -> FrameIdentity {
        self.producer_identity
    }

    #[must_use]
    pub fn consumer_identity(&self) -> FrameIdentity {
        self.consumer_identity
    }

    #[must_use]
    pub fn source_extent(&self) -> [u32; 2] {
        self.source_extent
    }

    #[must_use]
    pub fn cfa_pattern(&self) -> [u32; 4] {
        self.cfa_pattern
    }

    #[must_use]
    pub fn entries(&self) -> &[f32] {
        &self.entries
    }

    #[must_use]
    pub fn entry(&self, x: usize, y: usize) -> [f32; 2] {
        let index = (y * TINTLESS_MESH_WIDTH + x) * TINTLESS_MESH_CHANNELS;
        [self.entries[index], self.entries[index + 1]]
    }

    #[must_use]
    pub const fn valid_cells(&self) -> u32 {
        self.valid_cells
    }

    #[must_use]
    pub const fn qualified_components(&self) -> u32 {
        self.qualified_components
    }

    #[must_use]
    pub const fn residual_rms(&self) -> [f32; 2] {
        self.residual_rms
    }

    #[must_use]
    pub const fn clamp_count(&self) -> u32 {
        self.clamp_count
    }
}

#[derive(Clone, Copy)]
struct Cell {
    hue: [f32; 2],
    radius: f32,
    valid: bool,
    component: usize,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            hue: [0.0; 2],
            radius: 0.0,
            valid: false,
            component: usize::MAX,
        }
    }
}

/// Estimates the low-frequency radial R/G and B/G correction mesh.
///
/// # Errors
///
/// Returns a stable error for identity, geometry, CFA, sample, component, or
/// numeric violations. The producer may be the same frame or the exact
/// preceding sequence frame; run revisions must match.
pub fn estimate_gain_mesh(
    packet: &LcstStatisticsPacket,
    consumer_identity: FrameIdentity,
    source_extent: [u32; 2],
    cfa_pattern: [u32; 4],
) -> Result<TintlessGainMesh, TintlessError> {
    validate_contract(packet, consumer_identity, source_extent, cfa_pattern)?;
    let mut cells = [Cell::default(); CELL_COUNT];
    populate_cells(packet, &mut cells)?;
    let (component_count, valid_cells) = label_components(&mut cells);
    let qualified = qualify_components(&mut cells, component_count);
    if qualified == 0 {
        return Err(TintlessError::InsufficientComponents);
    }

    let mut knots = [[0.0_f32; RADIAL_KNOTS]; 2];
    let mut means = vec![[0.0_f32; 2]; qualified];
    let mut weights = [[1.0_f32; 2]; CELL_COUNT];
    for _ in 0..IRLS_ITERATIONS {
        update_knots(&cells, qualified, &weights, &mut knots)?;
        update_component_means(&cells, &knots, &weights, &mut means);
        update_robust_weights(&cells, &means, &knots, &mut weights);
    }
    if !knots.iter().flatten().all(|value| value.is_finite()) {
        return Err(TintlessError::NonFiniteSolve);
    }
    let residual_rms = residual_rms(&cells, &means, &knots);
    let (entries, clamp_count) = rasterize_gain_mesh(&knots);
    Ok(TintlessGainMesh {
        producer_identity: packet.identity(),
        consumer_identity,
        source_extent,
        cfa_pattern,
        entries: entries.into_boxed_slice(),
        valid_cells: valid_cells as u32,
        qualified_components: qualified as u32,
        residual_rms,
        clamp_count,
    })
}

fn validate_contract(
    packet: &LcstStatisticsPacket,
    consumer: FrameIdentity,
    extent: [u32; 2],
    cfa: [u32; 4],
) -> Result<(), TintlessError> {
    let producer = packet.identity();
    let same_frame = producer.frame_index == consumer.frame_index;
    let previous_frame = producer.frame_index.checked_add(1) == Some(consumer.frame_index);
    let run_matches = previous_frame || producer.run_revision == consumer.run_revision;
    if (!same_frame && !previous_frame)
        || !run_matches
        || producer.method_revision != consumer.method_revision
    {
        return Err(TintlessError::IdentityMismatch);
    }
    if packet.source_extent() != extent || extent.contains(&0) {
        return Err(TintlessError::ExtentMismatch);
    }
    if packet.cfa_pattern() != cfa || !valid_cfa_pattern(cfa) {
        return Err(TintlessError::CfaMismatch);
    }
    Ok(())
}

fn populate_cells(
    packet: &LcstStatisticsPacket,
    cells: &mut [Cell; CELL_COUNT],
) -> Result<(), TintlessError> {
    let [width, height] = packet.source_extent();
    let center = [(width - 1) as f32 * 0.5, (height - 1) as f32 * 0.5];
    let corner_radius = center[0].hypot(center[1]).max(f32::EPSILON);
    for y in 0..LCST_AVERAGE_GRID_HEIGHT {
        for x in 0..LCST_AVERAGE_GRID_WIDTH {
            let index = y * LCST_AVERAGE_GRID_WIDTH + x;
            let base = index * LCST_AVERAGE_CHANNELS;
            let values = &packet.average_rggb()[base..base + LCST_AVERAGE_CHANNELS];
            if !values.iter().all(|value| value.is_finite()) {
                return Err(TintlessError::NonFiniteAverage);
            }
            let green = 0.5 * (values[1] + values[2]);
            let finite_and_unsaturated = values
                .iter()
                .all(|value| *value >= 0.0 && *value < SATURATION_LIMIT);
            let hue = [
                (values[0].max(RATIO_EPSILON) / green.max(RATIO_EPSILON)).ln(),
                (values[3].max(RATIO_EPSILON) / green.max(RATIO_EPSILON)).ln(),
            ];
            let valid = green >= MINIMUM_GREEN
                && finite_and_unsaturated
                && hue.iter().all(|value| value.is_finite());
            let px = (x as f32 + 0.5) * width as f32 / LCST_AVERAGE_GRID_WIDTH as f32;
            let py = (y as f32 + 0.5) * height as f32 / LCST_AVERAGE_GRID_HEIGHT as f32;
            cells[index] = Cell {
                hue: if valid { hue } else { [0.0; 2] },
                radius: ((px - center[0]).hypot(py - center[1]) / corner_radius).clamp(0.0, 1.0),
                valid,
                component: usize::MAX,
            };
        }
    }
    Ok(())
}

fn label_components(cells: &mut [Cell; CELL_COUNT]) -> (usize, usize) {
    let mut queue = [0_usize; CELL_COUNT];
    let mut component = 0;
    let mut valid_cells = 0;
    for seed in 0..CELL_COUNT {
        if !cells[seed].valid || cells[seed].component != usize::MAX {
            continue;
        }
        cells[seed].component = component;
        let mut head = 0;
        let mut tail = 1;
        queue[0] = seed;
        while head < tail {
            let current = queue[head];
            head += 1;
            valid_cells += 1;
            let x = current % LCST_AVERAGE_GRID_WIDTH;
            let y = current / LCST_AVERAGE_GRID_WIDTH;
            for candidate in neighbor_indices(x, y).into_iter().flatten() {
                if cells[candidate].valid
                    && cells[candidate].component == usize::MAX
                    && hue_distance(cells[current].hue, cells[candidate].hue) <= HUE_LINK_THRESHOLD
                {
                    cells[candidate].component = component;
                    queue[tail] = candidate;
                    tail += 1;
                }
            }
        }
        component += 1;
    }
    (component, valid_cells)
}

fn neighbor_indices(x: usize, y: usize) -> [Option<usize>; 4] {
    [
        x.checked_sub(1).map(|nx| y * LCST_AVERAGE_GRID_WIDTH + nx),
        (x + 1 < LCST_AVERAGE_GRID_WIDTH).then_some(y * LCST_AVERAGE_GRID_WIDTH + x + 1),
        y.checked_sub(1).map(|ny| ny * LCST_AVERAGE_GRID_WIDTH + x),
        (y + 1 < LCST_AVERAGE_GRID_HEIGHT).then_some((y + 1) * LCST_AVERAGE_GRID_WIDTH + x),
    ]
}

fn hue_distance(left: [f32; 2], right: [f32; 2]) -> f32 {
    (left[0] - right[0]).abs().max((left[1] - right[1]).abs())
}

fn qualify_components(cells: &mut [Cell; CELL_COUNT], component_count: usize) -> usize {
    let mut counts = vec![0_usize; component_count];
    let mut min_radius = vec![f32::INFINITY; component_count];
    let mut max_radius = vec![f32::NEG_INFINITY; component_count];
    for cell in cells.iter().filter(|cell| cell.valid) {
        counts[cell.component] += 1;
        min_radius[cell.component] = min_radius[cell.component].min(cell.radius);
        max_radius[cell.component] = max_radius[cell.component].max(cell.radius);
    }
    let mut remap = vec![usize::MAX; component_count];
    let mut qualified = 0;
    for component in 0..component_count {
        if counts[component] >= MINIMUM_COMPONENT_CELLS
            && max_radius[component] - min_radius[component] >= MINIMUM_RADIAL_SPAN
        {
            remap[component] = qualified;
            qualified += 1;
        }
    }
    for cell in cells {
        if cell.valid {
            cell.component = remap[cell.component];
            cell.valid = cell.component != usize::MAX;
        }
    }
    qualified
}

fn sample_knots(knots: &[f32; RADIAL_KNOTS], radius: f32) -> f32 {
    let position = radius.clamp(0.0, 1.0) * (RADIAL_KNOTS - 1) as f32;
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(RADIAL_KNOTS - 1);
    let fraction = position - lower as f32;
    knots[lower] * (1.0 - fraction) + knots[upper] * fraction
}

fn update_component_means(
    cells: &[Cell; CELL_COUNT],
    knots: &[[f32; RADIAL_KNOTS]; 2],
    weights: &[[f32; 2]; CELL_COUNT],
    means: &mut [[f32; 2]],
) {
    let mut sums = vec![[0.0_f32; 2]; means.len()];
    let mut totals = vec![[0.0_f32; 2]; means.len()];
    for (index, cell) in cells.iter().enumerate().filter(|(_, cell)| cell.valid) {
        for channel in 0..2 {
            let weight = weights[index][channel];
            sums[cell.component][channel] +=
                weight * (cell.hue[channel] - sample_knots(&knots[channel], cell.radius));
            totals[cell.component][channel] += weight;
        }
    }
    for component in 0..means.len() {
        for channel in 0..2 {
            if totals[component][channel] > 0.0 {
                means[component][channel] = sums[component][channel] / totals[component][channel];
            }
        }
    }
}

fn update_knots(
    cells: &[Cell; CELL_COUNT],
    component_count: usize,
    weights: &[[f32; 2]; CELL_COUNT],
    knots: &mut [[f32; RADIAL_KNOTS]; 2],
) -> Result<(), TintlessError> {
    const UNKNOWN_COUNT: usize = RADIAL_KNOTS - 1;
    for channel in 0..2 {
        let mut component_weights = vec![0.0_f32; component_count];
        let mut component_hue = vec![0.0_f32; component_count];
        let mut component_basis = vec![[0.0_f32; UNKNOWN_COUNT]; component_count];
        for (index, cell) in cells.iter().enumerate().filter(|(_, cell)| cell.valid) {
            let weight = weights[index][channel];
            let basis = radial_basis(cell.radius);
            component_weights[cell.component] += weight;
            component_hue[cell.component] += weight * cell.hue[channel];
            for knot in 0..UNKNOWN_COUNT {
                component_basis[cell.component][knot] += weight * basis[knot];
            }
        }
        for component in 0..component_count {
            let total = component_weights[component];
            if total > 0.0 {
                component_hue[component] /= total;
                for knot in 0..UNKNOWN_COUNT {
                    component_basis[component][knot] /= total;
                }
            }
        }
        let mut normal = [[0.0_f32; UNKNOWN_COUNT]; UNKNOWN_COUNT];
        let mut rhs = [0.0_f32; UNKNOWN_COUNT];
        for (index, cell) in cells.iter().enumerate().filter(|(_, cell)| cell.valid) {
            let weight = weights[index][channel];
            let component = cell.component;
            let basis = radial_basis(cell.radius);
            let centered_hue = cell.hue[channel] - component_hue[component];
            for left in 0..UNKNOWN_COUNT {
                let left_basis = basis[left] - component_basis[component][left];
                rhs[left] += weight * left_basis * centered_hue;
                for right in 0..UNKNOWN_COUNT {
                    let right_basis = basis[right] - component_basis[component][right];
                    normal[left][right] += weight * left_basis * right_basis;
                }
            }
        }
        add_smoothness(&mut normal);
        let solution = solve_normal_equations(normal, rhs)?;
        knots[channel][0] = 0.0;
        knots[channel][1..].copy_from_slice(&solution);
    }
    Ok(())
}

fn add_smoothness(normal: &mut [[f32; RADIAL_KNOTS - 1]; RADIAL_KNOTS - 1]) {
    const UNKNOWN_COUNT: usize = RADIAL_KNOTS - 1;
    for center in 1..RADIAL_KNOTS - 1 {
        let mut second_difference = [0.0_f32; UNKNOWN_COUNT];
        if center > 1 {
            second_difference[center - 2] = 1.0;
        }
        second_difference[center - 1] = -2.0;
        second_difference[center] = 1.0;
        for left in 0..UNKNOWN_COUNT {
            for right in 0..UNKNOWN_COUNT {
                normal[left][right] +=
                    SMOOTHNESS_LAMBDA * second_difference[left] * second_difference[right];
            }
        }
    }
}

fn radial_basis(radius: f32) -> [f32; RADIAL_KNOTS - 1] {
    let mut basis = [0.0_f32; RADIAL_KNOTS - 1];
    let position = radius.clamp(0.0, 1.0) * (RADIAL_KNOTS - 1) as f32;
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(RADIAL_KNOTS - 1);
    let fraction = position - lower as f32;
    if lower > 0 {
        basis[lower - 1] += 1.0 - fraction;
    }
    if upper > 0 {
        basis[upper - 1] += fraction;
    }
    basis
}

fn solve_normal_equations(
    mut matrix: [[f32; RADIAL_KNOTS - 1]; RADIAL_KNOTS - 1],
    mut rhs: [f32; RADIAL_KNOTS - 1],
) -> Result<[f32; RADIAL_KNOTS - 1], TintlessError> {
    const N: usize = RADIAL_KNOTS - 1;
    for pivot in 0..N {
        let mut best = pivot;
        for row in pivot + 1..N {
            if matrix[row][pivot].abs() > matrix[best][pivot].abs() {
                best = row;
            }
        }
        let pivot_value = matrix[best][pivot];
        if !pivot_value.is_finite() || pivot_value.abs() <= f32::EPSILON {
            return Err(TintlessError::SingularSolve);
        }
        matrix.swap(pivot, best);
        rhs.swap(pivot, best);
        let scale = matrix[pivot][pivot];
        for column in pivot..N {
            matrix[pivot][column] /= scale;
        }
        rhs[pivot] /= scale;
        for row in 0..N {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot];
            for column in pivot..N {
                matrix[row][column] -= factor * matrix[pivot][column];
            }
            rhs[row] -= factor * rhs[pivot];
        }
    }
    if rhs.iter().all(|value| value.is_finite()) {
        Ok(rhs)
    } else {
        Err(TintlessError::NonFiniteSolve)
    }
}

fn update_robust_weights(
    cells: &[Cell; CELL_COUNT],
    means: &[[f32; 2]],
    knots: &[[f32; RADIAL_KNOTS]; 2],
    weights: &mut [[f32; 2]; CELL_COUNT],
) {
    for (index, cell) in cells.iter().enumerate() {
        if !cell.valid {
            weights[index] = [0.0; 2];
            continue;
        }
        for channel in 0..2 {
            let residual = cell.hue[channel]
                - means[cell.component][channel]
                - sample_knots(&knots[channel], cell.radius);
            let magnitude = residual.abs();
            weights[index][channel] = if magnitude <= HUBER_DELTA {
                1.0
            } else {
                HUBER_DELTA / magnitude
            };
        }
    }
}

fn residual_rms(
    cells: &[Cell; CELL_COUNT],
    means: &[[f32; 2]],
    knots: &[[f32; RADIAL_KNOTS]; 2],
) -> [f32; 2] {
    let mut squares = [0.0_f32; 2];
    let mut count = 0_u32;
    for cell in cells.iter().filter(|cell| cell.valid) {
        for channel in 0..2 {
            let residual = cell.hue[channel]
                - means[cell.component][channel]
                - sample_knots(&knots[channel], cell.radius);
            squares[channel] += residual * residual;
        }
        count += 1;
    }
    if count == 0 {
        return [0.0; 2];
    }
    [
        (squares[0] / count as f32).sqrt(),
        (squares[1] / count as f32).sqrt(),
    ]
}

fn rasterize_gain_mesh(knots: &[[f32; RADIAL_KNOTS]; 2]) -> (Vec<f32>, u32) {
    let center = [
        (TINTLESS_MESH_WIDTH - 1) as f32 * 0.5,
        (TINTLESS_MESH_HEIGHT - 1) as f32 * 0.5,
    ];
    let corner_radius = center[0].hypot(center[1]);
    let mut entries = Vec::with_capacity(TINTLESS_MESH_VALUES);
    let mut clamp_count = 0;
    for y in 0..TINTLESS_MESH_HEIGHT {
        for x in 0..TINTLESS_MESH_WIDTH {
            let radius = ((x as f32 - center[0]).hypot(y as f32 - center[1]) / corner_radius)
                .clamp(0.0, 1.0);
            for channel in 0..2 {
                let raw = (-sample_knots(&knots[channel], radius)).exp();
                let gain = raw.clamp(GAIN_MIN, GAIN_MAX);
                clamp_count += u32::from(gain != raw);
                entries.push(gain);
            }
        }
    }
    (entries, clamp_count)
}

#[cfg(test)]
mod tests {
    use super::{RADIAL_KNOTS, TintlessError, solve_normal_equations};

    #[test]
    fn singular_normal_equation_returns_stable_error() {
        let error = solve_normal_equations(
            [[0.0; RADIAL_KNOTS - 1]; RADIAL_KNOTS - 1],
            [0.0; RADIAL_KNOTS - 1],
        )
        .expect_err("singular normal equation must fail");

        assert_eq!(error, TintlessError::SingularSolve);
    }
}
