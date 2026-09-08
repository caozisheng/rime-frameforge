/// Clips near-neutral over-range highlights to white and scales chromatic highlights without hue rotation.
#[must_use]
pub fn shared_saturation_clip(rgb: [f32; 3]) -> [f32; 3] {
    let peak = rgb[0].max(rgb[1]).max(rgb[2]);
    if peak.is_finite() && peak > 1.0 {
        let scaled = [rgb[0] / peak, rgb[1] / peak, rgb[2] / peak];
        if scaled[0].min(scaled[1]).min(scaled[2]) >= 0.5 {
            [1.0; 3]
        } else {
            scaled
        }
    } else {
        rgb
    }
}
