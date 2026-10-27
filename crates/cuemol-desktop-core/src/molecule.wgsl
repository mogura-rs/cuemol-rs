struct Camera {
    view_proj: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    light: vec4<f32>,
    pixel_scale: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

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
        output.color = input.edge_color;
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
    let shade = 0.36 + 0.64 * max(dot(normal, normalize(camera.light.xyz)), 0.0);
    output.color = vec4<f32>(input.color.rgb * shade, input.color.a);
    if (input.selected > 0.5 && d > 0.62) {
        output.color = vec4<f32>(0.0, 0.95, 1.0, 1.0);
    }
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
};

struct BondVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_x: f32,
    @location(2) edge_ratio: f32,
    @location(3) edge_mode: f32,
    @location(4) edge_color: vec4<f32>,
    @location(5) perpendicular: vec3<f32>,
    @location(6) plane_world: vec3<f32>,
    @location(7) radius: f32,
};

@vertex
fn bond_vs(input: BondVsIn) -> BondVsOut {
    let start = input.from_radius.xyz;
    let finish = input.to_edge.xyz;
    var radius = input.from_radius.w;
    if input.edge_mode.z > 0.0 {
        let center_clip = camera.view_proj * vec4<f32>((start + finish) * 0.5, 1.0);
        let perspective_scale = select(1.0, center_clip.w / camera.pixel_scale.y, camera.pixel_scale.z > 0.5);
        radius = max(0.0001, input.edge_mode.z * 0.5 * camera.pixel_scale.x * perspective_scale);
    }
    let outer_radius = radius + input.to_edge.w;
    let axis = finish - start;
    let screen_axis = vec2<f32>(dot(axis, camera.right.xyz), dot(axis, camera.up.xyz));
    var perpendicular = camera.right.xyz;
    if (length(screen_axis) > 0.000001) {
        perpendicular = normalize(
            camera.right.xyz * (-screen_axis.y) + camera.up.xyz * screen_axis.x
        );
    }
    let t = (input.corner.y + 1.0) * 0.5;
    let world = mix(start, finish, t) + perpendicular * input.corner.x * outer_radius;
    var output: BondVsOut;
    output.position = camera.view_proj * vec4<f32>(world, 1.0);
    output.color = mix(input.color, input.end_color, t);
    output.local_x = input.corner.x * (outer_radius / radius);
    output.edge_ratio = outer_radius / radius;
    output.edge_mode = input.edge_mode.x;
    output.edge_color = input.edge_color;
    output.perpendicular = perpendicular;
    output.plane_world = world;
    output.radius = radius;
    return output;
}

@fragment
fn bond_fs(input: BondVsOut) -> PrimitiveFsOut {
    if (abs(input.local_x) > input.edge_ratio) {
        discard;
    }
    var output: PrimitiveFsOut;
    output.depth = input.position.z;
    if (abs(input.local_x) > 1.0) {
        output.color = vec4<f32>(input.edge_color.rgb, input.color.a);
        if (input.edge_mode > 1.5) {
            output.depth = 0.99;
        }
        return output;
    }
    let z = sqrt(max(0.0, 1.0 - input.local_x * input.local_x));
    let front = normalize(cross(camera.right.xyz, camera.up.xyz));
    let surface = camera.view_proj * vec4<f32>(input.plane_world + front * z * input.radius, 1.0);
    output.depth = surface.z / surface.w;
    if output.depth < 0.0 || output.depth > 1.0 { discard; }
    let normal = normalize(input.perpendicular * input.local_x + front * z);
    let shade = 0.36 + 0.64 * max(dot(normal, normalize(camera.light.xyz)), 0.0);
    output.color = vec4<f32>(input.color.rgb * shade, input.color.a);
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
};

@vertex
fn mesh_vs(input: MeshVsIn) -> MeshVsOut {
    var output: MeshVsOut;
    output.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    output.cull_back_faces = input.cull_back_faces;
    return output;
}

@fragment
fn mesh_fs(input: MeshVsOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    if (input.cull_back_faces > 0.5 && !front_facing) {
        discard;
    }
    if (input.cull_back_faces > 1.5) {
        return input.color;
    }
    let n = normalize(input.normal);
    let l = normalize(camera.light.xyz);
    // Two-sided lighting so tube ends and ribbon backs are not left black.
    let diff = abs(dot(n, l));
    let shade = 0.30 + 0.70 * diff;
    return vec4<f32>(input.color.rgb * shade, input.color.a);
}
