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

@group(0) @binding(0)
var<uniform> camera: Camera;

fn eye_normal(normal: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(normal, camera.right.xyz), dot(normal, camera.up.xyz), dot(normal, cross(camera.right.xyz, camera.up.xyz)));
}

fn finish_color(color: vec4<f32>, world: vec3<f32>) -> vec4<f32> {
    let distance = dot(world - camera.eye.xyz, cross(camera.right.xyz, camera.up.xyz));
    var result = reference_fog(color, distance, camera.fog.x, camera.fog.y, camera.background.rgb);
    if camera.pixel_scale.w > 0.5 {
        result = vec4<f32>(srgb_to_linear(result.rgb), result.a);
    }
    return result;
}

struct AtomVsIn {
    @location(0) corner: vec2<f32>,
    @location(1) center_radius: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) selected: vec4<f32>,
    @location(4) edge: vec4<f32>,
    @location(5) edge_color: vec4<f32>,
};

struct AtomVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) selected: f32,
    @location(3) edge_ratio: f32,
    @location(4) edge_mode: f32,
    @location(5) edge_color: vec4<f32>,
    @location(6) plane_world: vec3<f32>,
    @location(7) radius: f32,
};

@vertex
fn atom_vs(input: AtomVsIn) -> AtomVsOut {
    let center = input.center_radius.xyz;
    let radius = input.center_radius.w;
    let outer_radius = radius + input.edge.x;
    let world = center
        + camera.right.xyz * input.corner.x * outer_radius
        + camera.up.xyz * input.corner.y * outer_radius;
    var output: AtomVsOut;
    output.position = camera.view_proj * vec4<f32>(world, 1.0);
    output.local = input.corner * (outer_radius / radius);
    output.color = input.color;
    output.selected = input.selected.x;
    output.edge_ratio = outer_radius / radius;
    output.edge_mode = input.edge.y;
    output.edge_color = input.edge_color;
    output.plane_world = world;
    output.radius = radius;
    return output;
}

struct PrimitiveFsOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn atom_fs(input: AtomVsOut) -> PrimitiveFsOut {
    let d = dot(input.local, input.local);
    if (d > input.edge_ratio * input.edge_ratio) {
        discard;
    }
    var output: PrimitiveFsOut;
    output.depth = input.position.z;
    if (d > 1.0) {
        output.color = finish_color(input.edge_color, input.plane_world);
        if (input.edge_mode > 1.5) {
            output.depth = 0.99;
        }
        return output;
    }
    let z = sqrt(max(0.0, 1.0 - d));
    let front = normalize(cross(camera.right.xyz, camera.up.xyz));
    let surface = camera.view_proj * vec4<f32>(input.plane_world + front * z * input.radius, 1.0);
    output.depth = surface.z / surface.w;
    if output.depth < 0.0 || output.depth > 1.0 { discard; }
    let normal = normalize(
        camera.right.xyz * input.local.x + camera.up.xyz * input.local.y
            + normalize(cross(camera.right.xyz, camera.up.xyz)) * z
    );
    output.color = reference_lighting(eye_normal(normal), input.color);
    if (input.selected > 0.5 && d > 0.62) {
        output.color = vec4<f32>(0.0, 0.95, 1.0, 1.0);
    }
    output.color = finish_color(output.color, input.plane_world + front * z * input.radius);
    return output;
}

struct BondVsIn {
    @location(0) corner: vec2<f32>,
    @location(1) from_radius: vec4<f32>,
    @location(2) to_edge: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) end_color: vec4<f32>,
    @location(5) edge_color: vec4<f32>,
    @location(6) edge_mode: vec4<f32>,
    @location(7) screen: vec4<f32>,
};

struct BondVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
    @location(2) edge_ratio: f32,
    @location(3) edge_mode: f32,
    @location(4) edge_color: vec4<f32>,
    @location(5) perpendicular: vec3<f32>,
    @location(6) plane_world: vec3<f32>,
    @location(7) radius: f32,
    @location(8) along_normal: vec3<f32>,
    @location(9) cap: vec2<f32>,
    @location(10) unlit: f32,
    @location(11) line: vec2<f32>,
};

@vertex
fn bond_vs(input: BondVsIn) -> BondVsOut {
    let start = input.from_radius.xyz;
    let finish = input.to_edge.xyz;
    let radius = input.from_radius.w;
    if input.edge_mode.z > 0.0 {
        let start_clip = camera.view_proj * vec4<f32>(start, 1.0);
        let finish_clip = camera.view_proj * vec4<f32>(finish, 1.0);
        let screen_delta = (finish_clip.xy / finish_clip.w - start_clip.xy / start_clip.w) * camera.viewport.xy * 0.5;
        let length_pixels = length(screen_delta);
        let direction = screen_delta / max(length_pixels, 0.000001);
        let t = (input.corner.y + 1.0) * 0.5;
        let point = input.edge_mode.w > 0.5;
        var clip = mix(start_clip, finish_clip, t);
        var offset = vec2<f32>(-direction.y, direction.x) * input.corner.x * input.edge_mode.z * 0.5;
        if point {
            clip = start_clip;
            offset = input.corner * input.edge_mode.z * 0.5;
        }
        let screen_offset = vec2<f32>(input.screen.x, -input.screen.y) * clip.w;
        clip = vec4<f32>(clip.xy + (offset * start_clip.w + screen_offset) * 2.0 / camera.viewport.xy, clip.zw);
        var output: BondVsOut;
        output.position = clip;
        output.color = mix(input.color, input.end_color, select(t, 0.0, point));
        output.local = input.corner;
        output.edge_ratio = 1.0;
        output.edge_mode = 0.0;
        output.edge_color = input.edge_color;
        output.perpendicular = vec3<f32>(0.0);
        output.plane_world = mix(start, finish, select(t, 0.0, point));
        output.radius = 0.0;
        output.along_normal = vec3<f32>(0.0);
        output.cap = vec2<f32>(0.0);
        output.unlit = 1.0;
        output.line = vec2<f32>(t * length_pixels, select(input.edge_mode.y, 0.0, point));
        return output;
    }
    let outer_radius = radius + input.to_edge.w;
    let axis = finish - start;
    let front = normalize(cross(camera.right.xyz, camera.up.xyz));
    let screen_axis = vec2<f32>(dot(axis, camera.right.xyz), dot(axis, camera.up.xyz));
    let axis_length = max(length(axis), 0.000001);
    let screen_length = max(length(screen_axis), 0.000001);
    let axis_z = dot(axis, front);
    let along = (camera.right.xyz * screen_axis.x + camera.up.xyz * screen_axis.y) / screen_length;
    let perpendicular = camera.right.xyz * screen_axis.y / screen_length - camera.up.xyz * screen_axis.x / screen_length;
    let t = (input.corner.y + 1.0) * 0.5;
    let unlit = input.edge_mode.z > 0.0;
    let sin_phi = axis_z / axis_length;
    let cap_shift = select(2.0 * radius * abs(sin_phi) / screen_length, 0.0, unlit);
    var local = vec2<f32>(input.corner.x * (outer_radius / radius), input.corner.y);
    var world = mix(start, finish, t) + perpendicular * input.corner.x * outer_radius;
    if axis_z * (1.0 - 2.0 * t) > 0.0 && !unlit {
        world -= axis / axis_length * radius * axis_z / screen_length;
        local.y *= 1.0 + cap_shift;
    }
    var output: BondVsOut;
    output.position = camera.view_proj * vec4<f32>(world, 1.0);
    output.color = mix(input.color, input.end_color, t);
    output.local = local;
    output.edge_ratio = outer_radius / radius;
    output.edge_mode = input.edge_mode.x;
    output.edge_color = input.edge_color;
    output.perpendicular = perpendicular;
    output.plane_world = world;
    output.radius = radius * axis_length / screen_length;
    output.along_normal = -along * sin_phi + front * screen_length / axis_length;
    output.cap = vec2<f32>(cap_shift, -sign(axis_z));
    output.unlit = 0.0;
    output.line = vec2<f32>(0.0);
    return output;
}

@fragment
fn bond_fs(input: BondVsOut) -> PrimitiveFsOut {
    if (abs(input.local.x) > input.edge_ratio) {
        discard;
    }
    if input.unlit > 0.5 {
        if input.line.y > 0.5 && (u32(input.line.x) % 16u) < 8u { discard; }
        var output: PrimitiveFsOut;
        output.depth = input.position.z;
        output.color = finish_color(input.color, input.plane_world);
        return output;
    }
    let z = sqrt(max(0.0, 1.0 - input.local.x * input.local.x));
    let along = input.local.y - z * input.cap.x * input.cap.y;
    if abs(along) >= 1.0 { discard; }
    var output: PrimitiveFsOut;
    output.depth = input.position.z;
    if (abs(input.local.x) > 1.0) {
        output.color = finish_color(vec4<f32>(input.edge_color.rgb, input.color.a), input.plane_world);
        if (input.edge_mode > 1.5) {
            output.depth = 0.99;
        }
        return output;
    }
    let front = normalize(cross(camera.right.xyz, camera.up.xyz));
    let surface = camera.view_proj * vec4<f32>(input.plane_world + front * z * input.radius, 1.0);
    output.depth = surface.z / surface.w;
    if output.depth < 0.0 || output.depth > 1.0 { discard; }
    let normal = normalize(input.perpendicular * input.local.x + input.along_normal * z);
    output.color = finish_color(reference_lighting(eye_normal(normal), input.color), input.plane_world + front * z * input.radius);
    return output;
}

struct MeshVsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) cull_back_faces: f32,
};

struct MeshVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) cull_back_faces: f32,
    @location(3) world: vec3<f32>,
};

@vertex
fn mesh_vs(input: MeshVsIn) -> MeshVsOut {
    var output: MeshVsOut;
    output.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    if input.cull_back_faces < 1.5 {
        output.color = reference_lighting(eye_normal(input.normal), input.color);
    }
    output.cull_back_faces = input.cull_back_faces;
    output.world = input.position;
    return output;
}

@fragment
fn mesh_fs(input: MeshVsOut, @builtin(front_facing) front_facing: bool) -> PrimitiveFsOut {
    if (input.cull_back_faces > 0.5 && !front_facing) {
        discard;
    }
    var output: PrimitiveFsOut;
    output.color = finish_color(input.color, input.world);
    output.depth = select(input.position.z, 0.9999, input.cull_back_faces > 2.5);
    return output;
}
