// CueMol's FXAA 3.11 and SMAA passes. SMAA/Mol* notices are in licenses/.
fn luma(rgb: vec3<f32>) -> f32 { return sqrt(dot(rgb, vec3<f32>(0.299, 0.587, 0.114))); }
fn quality_step(index: i32) -> f32 {
    if index < 5 { return 1.0; }
    if index == 5 { return 1.5; }
    if index < 10 { return 2.0; }
    if index == 10 { return 4.0; }
    return 8.0;
}
@fragment
fn fxaa_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let pixel = params.pixel.xy;
    let color = color_at(uv).rgb;
    let center = luma(color);
    let down = luma(color_at(uv + vec2<f32>(0.0, -pixel.y)).rgb);
    let up = luma(color_at(uv + vec2<f32>(0.0, pixel.y)).rgb);
    let left = luma(color_at(uv + vec2<f32>(-pixel.x, 0.0)).rgb);
    let right = luma(color_at(uv + vec2<f32>(pixel.x, 0.0)).rgb);
    let minimum = min(center, min(min(down, up), min(left, right)));
    let maximum = max(center, max(max(down, up), max(left, right)));
    let range = maximum - minimum;
    if range < max(0.0312, maximum * 0.125) { return vec4<f32>(color, 1.0); }
    let dl = luma(color_at(uv - pixel).rgb);
    let ur = luma(color_at(uv + pixel).rgb);
    let ul = luma(color_at(uv + vec2<f32>(-pixel.x, pixel.y)).rgb);
    let dr = luma(color_at(uv + vec2<f32>(pixel.x, -pixel.y)).rgb);
    let down_up = down + up;
    let left_right = left + right;
    let left_corners = dl + ul;
    let down_corners = dl + dr;
    let right_corners = dr + ur;
    let up_corners = ur + ul;
    let edge_h = abs(-2.0 * left + left_corners) + abs(-2.0 * center + down_up) * 2.0 + abs(-2.0 * right + right_corners);
    let edge_v = abs(-2.0 * up + up_corners) + abs(-2.0 * center + left_right) * 2.0 + abs(-2.0 * down + down_corners);
    let horizontal = edge_h >= edge_v;
    let luma1 = select(left, down, horizontal);
    let luma2 = select(right, up, horizontal);
    let gradient1 = luma1 - center;
    let gradient2 = luma2 - center;
    let steepest1 = abs(gradient1) >= abs(gradient2);
    let gradient = 0.25 * max(abs(gradient1), abs(gradient2));
    var step_length = select(pixel.x, pixel.y, horizontal);
    var average = 0.5 * (luma2 + center);
    if steepest1 { step_length = -step_length; average = 0.5 * (luma1 + center); }
    var current = uv;
    if horizontal { current.y += step_length * 0.5; } else { current.x += step_length * 0.5; }
    let offset = select(vec2<f32>(0.0, pixel.y), vec2<f32>(pixel.x, 0.0), horizontal);
    var uv1 = current - offset;
    var uv2 = current + offset;
    var end1 = luma(color_at(uv1).rgb) - average;
    var end2 = luma(color_at(uv2).rgb) - average;
    var reached1 = abs(end1) >= gradient;
    var reached2 = abs(end2) >= gradient;
    if !reached1 { uv1 -= offset; }
    if !reached2 { uv2 += offset; }
    if !(reached1 && reached2) {
        for (var i = 2; i < 12; i += 1) {
            if !reached1 { end1 = luma(color_at(uv1).rgb) - average; }
            if !reached2 { end2 = luma(color_at(uv2).rgb) - average; }
            reached1 = abs(end1) >= gradient;
            reached2 = abs(end2) >= gradient;
            if !reached1 { uv1 -= offset * quality_step(i); }
            if !reached2 { uv2 += offset * quality_step(i); }
            if reached1 && reached2 { break; }
        }
    }
    let distance1 = select(uv.y - uv1.y, uv.x - uv1.x, horizontal);
    let distance2 = select(uv2.y - uv.y, uv2.x - uv.x, horizontal);
    let direction1 = distance1 < distance2;
    let offset_edge = -min(distance1, distance2) / (distance1 + distance2) + 0.5;
    let variation = (select(end2, end1, direction1) < 0.0) != (center < average);
    let neighborhood = (2.0 * (down_up + left_right) + left_corners + right_corners) / 12.0;
    let subpixel1 = clamp(abs(neighborhood - center) / range, 0.0, 1.0);
    let subpixel2 = (-2.0 * subpixel1 + 3.0) * subpixel1 * subpixel1;
    let offset_final = max(select(0.0, offset_edge, variation), subpixel2 * subpixel2 * 0.75);
    var final_uv = uv;
    if horizontal { final_uv.y += offset_final * step_length; } else { final_uv.x += offset_final * step_length; }
    return vec4<f32>(color_at(final_uv).rgb, 1.0);
}

fn max_color_delta(first: vec3<f32>, second: vec3<f32>) -> f32 {
    let delta = abs(first - second);
    return max(max(delta.r, delta.g), delta.b);
}
@fragment
fn smaa_edges_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let pixel = params.pixel.xy;
    let center = color_at(uv).rgb;
    let left = max_color_delta(center, color_at(uv - vec2<f32>(pixel.x, 0.0)).rgb);
    let top = max_color_delta(center, color_at(uv + vec2<f32>(0.0, pixel.y)).rgb);
    var edges = step(vec2<f32>(params.aa.x), vec2<f32>(left, top));
    if dot(edges, vec2<f32>(1.0)) == 0.0 { return vec4<f32>(0.0); }
    let right = max_color_delta(center, color_at(uv + vec2<f32>(pixel.x, 0.0)).rgb);
    let bottom = max_color_delta(center, color_at(uv - vec2<f32>(0.0, pixel.y)).rgb);
    let left2 = max_color_delta(center, color_at(uv - vec2<f32>(2.0 * pixel.x, 0.0)).rgb);
    let top2 = max_color_delta(center, color_at(uv + vec2<f32>(0.0, 2.0 * pixel.y)).rgb);
    let maximum = max(max(max(left, top), max(right, bottom)), max(left2, top2));
    edges *= step(vec2<f32>(0.5 * maximum), vec2<f32>(left, top));
    return vec4<f32>(edges, 0.0, 0.0);
}
fn search_length(edge: vec2<f32>, bias: f32) -> f32 {
    // Lookup tables retain their serialized row order.
    return 255.0 * textureSampleLevel(search_tex, nearest_sampler, vec2<f32>(bias + edge.r * 0.5, edge.g), 0.0).r;
}
fn search_left(start: vec2<f32>, end: f32) -> f32 {
    var coord = start;
    var edge = vec2<f32>(0.0, 1.0);
    for (var i = 0; i < 16; i += 1) {
        edge = color_at(coord).rg;
        coord.x -= 2.0 * params.pixel.x;
        if !(coord.x > end && edge.g > 0.8281 && edge.r == 0.0) { break; }
    }
    return coord.x + (3.25 - search_length(edge, 0.0)) * params.pixel.x;
}
fn search_right(start: vec2<f32>, end: f32) -> f32 {
    var coord = start;
    var edge = vec2<f32>(0.0, 1.0);
    for (var i = 0; i < 16; i += 1) {
        edge = color_at(coord).rg;
        coord.x += 2.0 * params.pixel.x;
        if !(coord.x < end && edge.g > 0.8281 && edge.r == 0.0) { break; }
    }
    return coord.x + (-3.25 + search_length(edge, 0.5)) * params.pixel.x;
}
fn search_up(start: vec2<f32>, end: f32) -> f32 {
    var coord = start;
    var edge = vec2<f32>(1.0, 0.0);
    for (var i = 0; i < 16; i += 1) {
        edge = color_at(coord).rg;
        coord.y += 2.0 * params.pixel.y;
        if !(coord.y > end && edge.r > 0.8281 && edge.g == 0.0) { break; }
    }
    return coord.y + (-3.25 + search_length(edge.gr, 0.0)) * params.pixel.y;
}
fn search_down(start: vec2<f32>, end: f32) -> f32 {
    var coord = start;
    var edge = vec2<f32>(1.0, 0.0);
    for (var i = 0; i < 16; i += 1) {
        edge = color_at(coord).rg;
        coord.y -= 2.0 * params.pixel.y;
        if !(coord.y < end && edge.r > 0.8281 && edge.g == 0.0) { break; }
    }
    return coord.y + (3.25 - search_length(edge.gr, 0.5)) * params.pixel.y;
}
fn smaa_area(distance: vec2<f32>, edge1: f32, edge2: f32) -> vec2<f32> {
    let coords = (16.0 * round(4.0 * vec2<f32>(edge1, edge2)) + distance + 0.5) / vec2<f32>(160.0, 560.0);
    return textureSampleLevel(area_tex, linear_sampler, coords, 0.0).rg;
}
@fragment
fn smaa_weights_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let pixel = params.pixel.xy;
    let pixel_coord = uv / pixel;
    let offset0 = uv.xyxy + pixel.xyxy * vec4<f32>(-0.25, 0.125, 1.25, 0.125);
    let offset1 = uv.xyxy + pixel.xyxy * vec4<f32>(-0.125, 0.25, -0.125, -1.25);
    let offset2 = vec4<f32>(offset0.xz, offset1.yw) + vec4<f32>(-2.0, 2.0, -2.0, 2.0) * pixel.xxyy * 16.0;
    let edge = color_at(uv).rg;
    var weights = vec4<f32>(0.0);
    if edge.g > 0.0 {
        let x1 = search_left(offset0.xy, offset2.x);
        let x2 = search_right(offset0.zw, offset2.y);
        let edge1 = color_at(vec2<f32>(x1, offset1.y)).r;
        let edge2 = color_at(vec2<f32>(x2 + pixel.x, offset1.y - pixel.y)).r;
        let distance = sqrt(abs(vec2<f32>(x1, x2) / pixel.x - pixel_coord.x));
        let area = smaa_area(distance, edge1, edge2);
        weights.r = area.x;
        weights.g = area.y;
    }
    if edge.r > 0.0 {
        let y1 = search_up(offset1.xy, offset2.z);
        let y2 = search_down(offset1.zw, offset2.w);
        let edge1 = color_at(vec2<f32>(offset0.x, y1)).g;
        let edge2 = color_at(vec2<f32>(offset0.x, y2)).g;
        let distance = sqrt(abs(vec2<f32>(y1, y2) / pixel.y - pixel_coord.y));
        let area = smaa_area(distance, edge1, edge2);
        weights.b = area.x;
        weights.a = area.y;
    }
    return weights;
}
@fragment
fn smaa_blend_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let pixel = params.pixel.xy;
    let center = aux_at(uv);
    let weights = vec4<f32>(center.x, aux_at(uv - vec2<f32>(0.0, pixel.y)).g, center.z, aux_at(uv + vec2<f32>(pixel.x, 0.0)).a);
    let color = color_at(uv);
    if dot(weights, vec4<f32>(1.0)) < 1.0e-5 { return color; }
    var direction = vec2<f32>(select(-weights.b, weights.a, weights.a > weights.b), select(weights.r, -weights.g, weights.g > weights.r));
    if abs(direction.x) > abs(direction.y) { direction.y = 0.0; } else { direction.x = 0.0; }
    let opposite = color_at(uv + sign(direction) * pixel);
    let weight = max(abs(direction.x), abs(direction.y));
    let mixed = mix(vec4<f32>(pow(color.rgb, vec3<f32>(2.2)), color.a), vec4<f32>(pow(opposite.rgb, vec3<f32>(2.2)), opposite.a), weight);
    return vec4<f32>(pow(mixed.rgb, vec3<f32>(1.0 / 2.2)), mixed.a);
}
