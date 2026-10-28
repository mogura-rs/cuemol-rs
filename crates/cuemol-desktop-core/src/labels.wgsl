const OUTPUT_SRGB: bool = false;
@group(0) @binding(0) var glyph: texture_2d<f32>;
@group(0) @binding(1) var depth_map: texture_2d<f32>;
@group(0) @binding(2) var glyph_sampler: sampler;
struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) scene_uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) @interpolate(flat) depth: f32,
};
@vertex
fn vs(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> Vertex {
    var out: Vertex;
    out.position = vec4<f32>(position.xy, 0.0, 1.0);
    out.uv = uv;
    out.scene_uv = vec2<f32>(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
    out.color = color;
    out.depth = position.z;
    return out;
}
@fragment
fn fs(in: Vertex) -> @location(0) vec4<f32> {
    let dimensions = vec2<i32>(textureDimensions(depth_map));
    let pixel = clamp(vec2<i32>(in.scene_uv * vec2<f32>(dimensions)), vec2<i32>(0), dimensions - vec2<i32>(1));
    let packed = round(textureLoad(depth_map, pixel, 0).rgb * 255.0);
    let scene_depth = dot(packed, vec3<f32>(65536.0, 256.0, 1.0)) / 16777215.0;
    if in.depth > scene_depth + 0.00000012 { discard; }
    let coverage = textureSample(glyph, glyph_sampler, in.uv).r;
    var color = in.color.rgb;
    if OUTPUT_SRGB {
        color = select(pow((color + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), color / 12.92, color <= vec3<f32>(0.04045));
    }
    return vec4<f32>(color, coverage * in.color.a);
}
