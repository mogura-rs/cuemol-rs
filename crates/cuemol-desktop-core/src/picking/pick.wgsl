struct PickFsOut {
    @location(0) id_depth: vec4<u32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn atom_pick_fs(input: AtomVsOut) -> PickFsOut {
    let result = atom_fragment(input);
    return PickFsOut(vec4<u32>(u32(input.color.r), bitcast<u32>(result.depth), 0u, 0u), result.depth);
}

@fragment
fn bond_pick_fs(input: BondVsOut) -> PickFsOut {
    let result = bond_fragment(input);
    let id = select(input.edge_color.r, input.edge_color.g, input.local.y > 0.0);
    return PickFsOut(vec4<u32>(u32(id), bitcast<u32>(result.depth), 0u, 0u), result.depth);
}

struct MeshPickOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) id: u32,
    @location(1) @interpolate(flat) cull: f32,
};

@vertex
fn mesh_pick_vs(input: MeshVsIn) -> MeshPickOut {
    return MeshPickOut(camera.view_proj * vec4<f32>(input.position, 1.0), u32(input.color.r), input.cull_back_faces);
}

@fragment
fn mesh_pick_fs(input: MeshPickOut, @builtin(front_facing) front: bool) -> PickFsOut {
    if input.cull > 0.5 && !front { discard; }
    let depth = input.position.z;
    return PickFsOut(vec4<u32>(input.id, bitcast<u32>(depth), 0u, 0u), depth);
}
