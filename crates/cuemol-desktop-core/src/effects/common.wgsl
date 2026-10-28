struct Parameters {
    depth: vec4<f32>,
    ndc: vec4<f32>,
    pixel: vec4<f32>,
    ao: vec4<f32>,
    misc: vec4<f32>,
    aa: vec4<f32>,
    output: vec4<f32>,
};
@group(0) @binding(0) var<uniform> params: Parameters;
@group(0) @binding(1) var color_tex: texture_2d<f32>;
@group(0) @binding(2) var aux_tex: texture_2d<f32>;
@group(0) @binding(3) var depth_tex: texture_2d<f32>;
@group(0) @binding(4) var normal_tex: texture_2d<f32>;
@group(0) @binding(5) var linear_sampler: sampler;
@group(0) @binding(6) var nearest_sampler: sampler;
@group(0) @binding(7) var area_tex: texture_2d<f32>;
@group(0) @binding(8) var search_tex: texture_2d<f32>;

struct Fullscreen {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
@vertex
fn fullscreen_vs(@builtin(vertex_index) index: u32) -> Fullscreen {
    let point = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0))[index];
    return Fullscreen(vec4<f32>(point, 0.0, 1.0), point * 0.5 + 0.5);
}

// The reference passes use bottom-up UV; wgpu textures use top-down rows.
fn tex_uv(uv: vec2<f32>) -> vec2<f32> { return vec2<f32>(uv.x, 1.0 - uv.y); }
fn color_at(uv: vec2<f32>) -> vec4<f32> { return textureSampleLevel(color_tex, linear_sampler, tex_uv(uv), 0.0); }
fn color_nearest(uv: vec2<f32>) -> vec4<f32> { return textureSampleLevel(color_tex, nearest_sampler, tex_uv(uv), 0.0); }
fn aux_at(uv: vec2<f32>) -> vec4<f32> { return textureSampleLevel(aux_tex, linear_sampler, tex_uv(uv), 0.0); }
fn aux_nearest(uv: vec2<f32>) -> vec4<f32> { return textureSampleLevel(aux_tex, nearest_sampler, tex_uv(uv), 0.0); }
fn texel(uv: vec2<f32>, size: vec2<u32>) -> vec2<i32> {
    return clamp(vec2<i32>(tex_uv(uv) * vec2<f32>(size)), vec2<i32>(0), vec2<i32>(size) - 1);
}
fn depth_at(uv: vec2<f32>) -> f32 {
    let bytes = vec3<u32>(round(textureLoad(depth_tex, texel(uv, textureDimensions(depth_tex)), 0).rgb * 255.0));
    return f32((bytes.x << 16u) | (bytes.y << 8u) | bytes.z) / 16777215.0;
}
fn normal_at(uv: vec2<f32>) -> vec3<f32> { return textureLoad(normal_tex, texel(uv, textureDimensions(normal_tex)), 0).xyz; }
fn linear_depth(raw: f32) -> f32 {
    if params.depth.z > 0.5 { return params.depth.x + raw * params.depth.y; }
    return params.depth.x / (params.depth.y - raw);
}
fn view_position(uv: vec2<f32>, z: f32) -> vec3<f32> {
    var xy = params.ndc.xy * uv + params.ndc.zw;
    if params.depth.z < 0.5 { xy *= z; }
    return vec3<f32>(xy, z);
}
fn camera_direction(position: vec3<f32>) -> vec3<f32> {
    if params.depth.z > 0.5 { return vec3<f32>(0.0, 0.0, -1.0); }
    return normalize(-position);
}
@fragment
fn copy_fs(input: Fullscreen) -> @location(0) vec4<f32> { return color_at(input.uv); }
@fragment
fn accumulate_fs(input: Fullscreen) -> @location(0) vec4<f32> { return color_at(input.uv) * params.aa.z; }
@fragment
fn present_fs(input: Fullscreen) -> @location(0) vec4<f32> {
    var color = color_at(input.uv);
    if params.output.y > 0.5 { color *= params.aa.w; }
    if params.output.x > 0.5 { color = vec4<f32>(srgb_to_linear(color.rgb), color.a); }
    return color;
}
