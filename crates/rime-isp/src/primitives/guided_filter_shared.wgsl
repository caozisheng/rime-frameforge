fn gf_inside(x: i32, y: i32, size: vec2<i32>) -> bool {
  return x >= 0 && y >= 0 && x < size.x && y < size.y;
}

fn gf_local_variance(texture: texture_2d<f32>, center: vec2<i32>, radius: i32) -> f32 {
  let size = vec2<i32>(textureDimensions(texture));
  var sum = 0.0;
  var sum_sq = 0.0;
  var count = 0.0;
  for (var dy = -radius; dy <= radius; dy += 1) {
    for (var dx = -radius; dx <= radius; dx += 1) {
      let position = center + vec2<i32>(dx, dy);
      if (gf_inside(position.x, position.y, size)) {
        let value = textureLoad(texture, position, 0).x;
        sum += value;
        sum_sq += value * value;
        count += 1.0;
      }
    }
  }
  let mean = sum / max(count, 1.0);
  return max(sum_sq / max(count, 1.0) - mean * mean, 0.0);
}

fn gf_gradient_chi(texture: texture_2d<f32>, center: vec2<i32>) -> f32 {
  return sqrt(abs(gf_local_variance(texture, center, 1) * gf_local_variance(texture, center, 3)));
}

fn gf_dynamic_epsilon(texture: texture_2d<f32>, center: vec2<i32>) -> f32 {
  let size = vec2<i32>(textureDimensions(texture));
  var minimum = 1e30;
  var maximum = -1e30;
  for (var dy = -3; dy <= 3; dy += 1) {
    for (var dx = -3; dx <= 3; dx += 1) {
      let position = center + vec2<i32>(dx, dy);
      if (gf_inside(position.x, position.y, size)) {
        let value = textureLoad(texture, position, 0).x;
        minimum = min(minimum, value);
        maximum = max(maximum, value);
      }
    }
  }
  let dynamic_range = max(maximum - minimum, 0.0);
  return max((0.001 * dynamic_range) * (0.001 * dynamic_range), 1e-12);
}

fn gf_gradient_weight(texture: texture_2d<f32>, center: vec2<i32>) -> f32 {
  let size = vec2<i32>(textureDimensions(texture));
  let chi = gf_gradient_chi(texture, center);
  let epsilon = gf_dynamic_epsilon(texture, center);
  var mean = 0.0;
  var count = 0.0;
  for (var dy = -3; dy <= 3; dy += 1) {
    for (var dx = -3; dx <= 3; dx += 1) {
      let position = center + vec2<i32>(dx, dy);
      if (gf_inside(position.x, position.y, size)) {
        mean += gf_gradient_chi(texture, position);
        count += 1.0;
      }
    }
  }
  return (chi + epsilon) / (mean / max(count, 1.0) + epsilon);
}

fn gf_gradient_gamma(texture: texture_2d<f32>, center: vec2<i32>) -> f32 {
  let chi = gf_gradient_chi(texture, center);
  let size = vec2<i32>(textureDimensions(texture));
  var mean = 0.0;
  var minimum = 1e30;
  var count = 0.0;
  for (var dy = -3; dy <= 3; dy += 1) {
    for (var dx = -3; dx <= 3; dx += 1) {
      let position = center + vec2<i32>(dx, dy);
      if (gf_inside(position.x, position.y, size)) {
        let local_chi = gf_gradient_chi(texture, position);
        mean += local_chi;
        minimum = min(minimum, local_chi);
        count += 1.0;
      }
    }
  }
  let average = mean / max(count, 1.0);
  let denominator = max(average - minimum, 1e-6);
  let gamma = 1.0 - 1.0 / (1.0 + exp(4.0 * (chi - average) / denominator));
  return clamp(gamma, 0.0, 1.0);
}

fn gf_box_moments(texture: texture_2d<f32>, position: vec2<i32>, size: vec2<i32>) -> vec2<f32> {
  var sum = 0.0;
  var sum_sq = 0.0;
  var count = 0.0;
  for (var dy = -3; dy <= 3; dy += 1) {
    for (var dx = -3; dx <= 3; dx += 1) {
      let sample_position = position + vec2<i32>(dx, dy);
      if (gf_inside(sample_position.x, sample_position.y, size)) {
        let value = textureLoad(texture, sample_position, 0).x;
        sum += value;
        sum_sq += value * value;
        count += 1.0;
      }
    }
  }
  return vec2<f32>(sum / max(count, 1.0), sum_sq / max(count, 1.0));
}
