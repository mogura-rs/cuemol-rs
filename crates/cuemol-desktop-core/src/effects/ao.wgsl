// CueMol's XeGTAO horizon integration and edge-aware filtering. See licenses/xegtao.txt.
fn gradient_noise(point: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(point, vec2<f32>(0.06711056, 0.00583715))));
}
fn packed_edges(center: f32, left: f32, right: f32, top: f32, bottom: f32) -> f32 {
    var edges = vec4<f32>(left, right, top, bottom) - center;
    let slope_lr = (edges.y - edges.x) * 0.5;
    let slope_tb = (edges.w - edges.z) * 0.5;
    let adjusted = edges + vec4<f32>(slope_lr, -slope_lr, slope_tb, -slope_tb);
    edges = min(abs(edges), abs(adjusted));
    edges = clamp(1.25 - edges / (center * 0.011), vec4<f32>(0.0), vec4<f32>(1.0));
    return dot(round(edges * 2.9), vec4<f32>(64.0, 16.0, 4.0, 1.0) / 255.0);
}
@fragment
fn gtao_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let raw_depth = depth_at(uv);
    let stored = normal_at(uv);
    if raw_depth >= 1.0 || dot(stored, stored) < 0.5 || params.ao.x <= 0.0 {
        return vec4<f32>(1.0);
    }
    var z = linear_depth(raw_depth);
    var center = view_position(uv, z);
    var normal = normalize(vec3<f32>(stored.xy, -stored.z));
    let direction = camera_direction(center);
    let facing = dot(normal, direction);
    if facing < 0.0 { normal = normalize(normal - 1.01 * facing * direction); }
    let pixel = params.pixel.zw;
    let edges = packed_edges(z,
        linear_depth(depth_at(uv - vec2<f32>(pixel.x, 0.0))),
        linear_depth(depth_at(uv + vec2<f32>(pixel.x, 0.0))),
        linear_depth(depth_at(uv + vec2<f32>(0.0, pixel.y))),
        linear_depth(depth_at(uv - vec2<f32>(0.0, pixel.y))));
    z *= 0.99999;
    center = view_position(uv, z);
    let view_direction = camera_direction(center);
    var pixel_size = params.ndc.x * pixel.x;
    if params.depth.z < 0.5 { pixel_size *= z; }
    let screen_radius = min(params.ao.x / max(pixel_size, 1.0e-6), params.misc.y);
    let radius = screen_radius * pixel_size;
    let falloff_range = 0.615 * radius;
    let falloff_from = radius * (1.0 - 0.615);
    let falloff_mul = -1.0 / falloff_range;
    let falloff_add = falloff_from / falloff_range + 1.0;
    let pixel_center = uv / pixel;
    let slice_noise = fract(gradient_noise(pixel_center) + params.misc.x);
    let sample_noise = fract(slice_noise + dot(pixel_center, vec2<f32>(0.7548776662, 0.5698402909)));
    let minimum_step = 1.3 / screen_radius;
    let slices = clamp(i32(params.ao.z), 1, 16);
    let steps = clamp(i32(params.ao.w), 1, 16);
    var visibility = 0.0;
    for (var slice = 0; slice < slices; slice += 1) {
        let phi = (f32(slice) + slice_noise) / f32(slices) * 3.14159265359;
        let cos_phi = cos(phi);
        let sin_phi = sin(phi);
        let omega = vec2<f32>(cos_phi, sin_phi) * screen_radius;
        let slice_direction = vec3<f32>(cos_phi, sin_phi, 0.0);
        let ortho_direction = slice_direction - dot(slice_direction, view_direction) * view_direction;
        let axis = normalize(cross(ortho_direction, view_direction));
        let projected_normal = normal - axis * dot(normal, axis);
        let sign_normal = sign(dot(ortho_direction, projected_normal));
        var normal_length = length(projected_normal);
        let cos_normal = clamp(dot(projected_normal, view_direction) / max(normal_length, 1.0e-6), 0.0, 1.0);
        let angle = sign_normal * acos(cos_normal);
        let low0 = cos(angle + 1.57079632679);
        let low1 = cos(angle - 1.57079632679);
        var horizon0 = low0;
        var horizon1 = low1;
        for (var step = 0; step < steps; step += 1) {
            let noise = fract(sample_noise + f32(slice + step * steps) * 0.6180339887);
            var sample_distance = (f32(step) + noise) / f32(steps);
            sample_distance = pow(sample_distance, 2.0) + minimum_step;
            let offset = round(sample_distance * omega) * pixel;
            let uv0 = uv + offset;
            let uv1 = uv - offset;
            let delta0 = view_position(uv0, linear_depth(depth_at(uv0))) - center;
            let delta1 = view_position(uv1, linear_depth(depth_at(uv1))) - center;
            let distance0 = length(delta0);
            let distance1 = length(delta1);
            var weight0 = clamp(distance0 * falloff_mul + falloff_add, 0.0, 1.0);
            var weight1 = clamp(distance1 * falloff_mul + falloff_add, 0.0, 1.0);
            if weight0 > 0.0 {
                let sample_normal = normal_at(uv0);
                if dot(sample_normal, sample_normal) < 0.5 { weight0 = 0.0; }
            }
            if weight1 > 0.0 {
                let sample_normal = normal_at(uv1);
                if dot(sample_normal, sample_normal) < 0.5 { weight1 = 0.0; }
            }
            let horizon_sample0 = dot(delta0 / max(distance0, 1.0e-6), view_direction);
            let horizon_sample1 = dot(delta1 / max(distance1, 1.0e-6), view_direction);
            horizon0 = max(horizon0, mix(low0, horizon_sample0, weight0));
            horizon1 = max(horizon1, mix(low1, horizon_sample1, weight1));
        }
        normal_length = mix(normal_length, 1.0, 0.05);
        let h0 = -acos(clamp(horizon1, -1.0, 1.0));
        let h1 = acos(clamp(horizon0, -1.0, 1.0));
        let arc0 = (cos_normal + 2.0 * h0 * sin(angle) - cos(2.0 * h0 - angle)) / 4.0;
        let arc1 = (cos_normal + 2.0 * h1 * sin(angle) - cos(2.0 * h1 - angle)) / 4.0;
        visibility += normal_length * (arc0 + arc1);
    }
    visibility = max(0.03, pow(max(visibility / f32(slices), 0.0), params.ao.y));
    return vec4<f32>(visibility, edges, 0.0, 1.0);
}

fn unpack_edges(packed: f32) -> vec4<f32> {
    let value = u32(packed * 255.5);
    return vec4<f32>(f32((value >> 6u) & 3u), f32((value >> 4u) & 3u), f32((value >> 2u) & 3u), f32(value & 3u)) / 3.0;
}
@fragment
fn denoise_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let e = params.pixel.zw;
    let center = color_nearest(uv).rg;
    let edges = unpack_edges(center.g);
    let left = color_nearest(uv - vec2<f32>(e.x, 0.0)).rg;
    let right = color_nearest(uv + vec2<f32>(e.x, 0.0)).rg;
    let top = color_nearest(uv + vec2<f32>(0.0, e.y)).rg;
    let bottom = color_nearest(uv - vec2<f32>(0.0, e.y)).rg;
    let tl = color_nearest(uv + vec2<f32>(-e.x, e.y)).r;
    let tr = color_nearest(uv + vec2<f32>(e.x, e.y)).r;
    let bl = color_nearest(uv - e).r;
    let br = color_nearest(uv + vec2<f32>(e.x, -e.y)).r;
    let w = edges * vec4<f32>(unpack_edges(left.g).y, unpack_edges(right.g).x, unpack_edges(top.g).w, unpack_edges(bottom.g).z);
    let diagonal = 0.425 * vec4<f32>(edges.x * edges.z, edges.y * edges.z, edges.x * edges.w, edges.y * edges.w);
    let sum = 1.2 * center.r + dot(w, vec4<f32>(left.r, right.r, top.r, bottom.r)) + dot(diagonal, vec4<f32>(tl, tr, bl, br));
    let weight = 1.2 + dot(w + diagonal, vec4<f32>(1.0));
    return vec4<f32>(sum / weight, center.g, 0.0, 1.0);
}

fn upsample_ao(uv: vec2<f32>) -> f32 {
    let z = linear_depth(depth_at(uv));
    let grid = uv / params.pixel.zw - 0.5;
    let base = floor(grid);
    let fraction = grid - base;
    let start = (base + 0.5) * params.pixel.zw;
    let px = vec2<f32>(params.pixel.z, 0.0);
    let py = vec2<f32>(0.0, params.pixel.w);
    let coords = array<vec2<f32>, 4>(start, start + px, start + py, start + px + py);
    var depths: vec4<f32>;
    var values: vec4<f32>;
    for (var i = 0u; i < 4u; i += 1u) {
        depths[i] = linear_depth(depth_at(coords[i]));
        values[i] = aux_nearest(coords[i]).r;
    }
    let minimum = min(min(depths.x, depths.y), min(depths.z, depths.w));
    let maximum = max(max(depths.x, depths.y), max(depths.z, depths.w));
    if maximum - minimum <= max(abs(z), 1.0e-4) * 0.03 {
        let weights = vec4<f32>((1.0 - fraction.x) * (1.0 - fraction.y), fraction.x * (1.0 - fraction.y), (1.0 - fraction.x) * fraction.y, fraction.x * fraction.y);
        return dot(weights, values);
    }
    var closest = abs(depths.x - z);
    var result = values.x;
    for (var i = 1u; i < 4u; i += 1u) {
        let distance = abs(depths[i] - z);
        if distance < closest { closest = distance; result = values[i]; }
    }
    return result;
}
@fragment
fn composite_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    let color = color_at(input.uv);
    if params.depth.w < 0.5 { return color; }
    var ao = aux_nearest(input.uv).r;
    if params.aa.y > 0.5 { ao = upsample_ao(input.uv); }
    let visibility = clamp((params.misc.z - linear_depth(depth_at(input.uv))) * params.misc.w, 0.0, 1.0);
    return vec4<f32>(color.rgb * mix(1.0, ao, visibility), color.a);
}
