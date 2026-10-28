struct HoverUniform {
    fill: vec4<f32>,
    tones: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(0) var ids: texture_2d<u32>;
@group(0) @binding(1) var mask: texture_2d<f32>;
@group(0) @binding(2) var mask_sampler: sampler;
@group(0) @binding(3) var<uniform> hover: HoverUniform;
struct HoverVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
@vertex fn hover_vs(@builtin(vertex_index) vertex: u32) -> HoverVertex {
    let p = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    return HoverVertex(vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0), vec2<f32>(p.x, 1.0 - p.y));
}
@fragment fn mask_horizontal(input: HoverVertex) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(ids));
    let center = vec2<i32>(input.uv * vec2<f32>(size));
    let radius = i32(hover.params.z);
    let k2 = -0.5 / (hover.params.y * hover.params.y);
    var sum = 0.0; var weights = 0.0;
    for (var k = -radius; k <= radius; k += 1) {
        let w = exp(f32(k * k) * k2);
        let p = clamp(center + vec2<i32>(k, 0), vec2<i32>(0), size - 1);
        sum += w * select(0.0, 1.0, textureLoad(ids, p, 0).x == u32(hover.params.x));
        weights += w;
    }
    return vec4<f32>(vec3<f32>(sum / weights), 1.0);
}
@fragment fn mask_vertical(input: HoverVertex) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(mask));
    let center = vec2<i32>(input.uv * vec2<f32>(size));
    let radius = i32(hover.params.z);
    let k2 = -0.5 / (hover.params.y * hover.params.y);
    var sum = 0.0; var weights = 0.0;
    for (var k = -radius; k <= radius; k += 1) {
        let w = exp(f32(k * k) * k2);
        let p = clamp(center + vec2<i32>(0, k), vec2<i32>(0), size - 1);
        sum += w * textureLoad(mask, p, 0).r;
        weights += w;
    }
    return vec4<f32>(vec3<f32>(sum / weights), 1.0);
}
@fragment fn hover_overlay(input: HoverVertex) -> @location(0) vec4<f32> {
    let s = textureSampleLevel(mask, mask_sampler, input.uv, 0.0).r;
    if s <= 0.10 { discard; }
    let band = smoothstep(0.10, 0.22, s) * (1.0 - smoothstep(0.78, 0.90, s));
    let tone = smoothstep(0.42, 0.58, s);
    let edge_rgb = vec3<f32>(mix(hover.tones.x, hover.tones.y, tone));
    let edge_a = 0.9 * band;
    let fill_a = hover.fill.a * smoothstep(0.45, 0.55, s);
    let a = edge_a + fill_a * (1.0 - edge_a);
    var rgb = (edge_rgb * edge_a + hover.fill.rgb * fill_a * (1.0 - edge_a)) / max(a, 0.00001);
    if hover.params.w > 0.5 { rgb = srgb_to_linear(rgb); }
    return vec4<f32>(rgb, a);
}
