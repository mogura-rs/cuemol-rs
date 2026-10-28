struct Camera {
    view_proj: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    eye: vec4<f32>,
    background: vec4<f32>,
    fog: vec4<f32>,
    pixel_scale: vec4<f32>,
    viewport: vec4<f32>,
};
struct Volume {
    world_to_texture: mat4x4<f32>,
    lower: vec4<f32>,
    span: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> volume: Volume;
@group(1) @binding(1) var density: texture_3d<f32>;
@group(1) @binding(2) var transfer: texture_2d<f32>;
@group(1) @binding(3) var linear: sampler;
struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) coordinate: vec3<f32>,
    @location(1) depth: f32,
};
@vertex fn vs(@builtin(vertex_index) vertex: u32, @builtin(instance_index) plane: u32) -> Vertex {
    let corners = array<vec2<f32>, 6>(vec2(0.,0.), vec2(1.,0.), vec2(0.,1.), vec2(0.,1.), vec2(1.,0.), vec2(1.,1.));
    let xy = volume.lower.xy + corners[vertex] * volume.span.xy;
    let z = volume.lower.z + f32(plane) * volume.lower.w;
    let front = cross(camera.right.xyz, camera.up.xyz);
    let world = vec4(camera.eye.xyz + camera.right.xyz * xy.x + camera.up.xyz * xy.y + front * z, 1.);
    var result: Vertex;
    result.position = camera.view_proj * world;
    result.coordinate = (volume.world_to_texture * world).xyz;
    result.depth = -z;
    return result;
}
@fragment fn fs(input: Vertex) -> @location(0) vec4<f32> {
    if input.depth > camera.fog.x { discard; }
    if any(input.coordinate < vec3(0.)) || any(input.coordinate > vec3(1.)) { discard; }
    let value = textureSample(density, linear, input.coordinate).r;
    var color = textureSample(transfer, linear, vec2(value, 0.5));
    color.a = clamp(color.a * volume.lower.w * volume.span.z, 0., 1.);
    if color.a <= 0. { discard; }
    if volume.span.w > 0.5 {
        color = vec4(select(pow((color.rgb + vec3(0.055)) / 1.055, vec3(2.4)), color.rgb / 12.92, color.rgb <= vec3(0.04045)), color.a);
    }
    return color;
}
