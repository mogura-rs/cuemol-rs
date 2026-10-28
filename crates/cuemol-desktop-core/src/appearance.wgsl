fn reference_lighting(normal: vec3<f32>, color: vec4<f32>) -> vec4<f32> {
    let light = vec3<f32>(1.0, 1.0, 1.5);
    let n = normal / max(length(normal), 1.0e-12);
    let diffuse = max(dot(n, normalize(light)), 0.0);
    let half_vector = normalize(light + vec3<f32>(0.0, 0.0, 1.0));
    let specular = select(0.0, pow(max(dot(n, half_vector), 0.0), 32.0), diffuse > 0.0);
    return vec4<f32>(clamp(color.rgb * (0.4 + 0.8 * diffuse) + vec3<f32>(0.16 * specular), vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
}

fn reference_fog(color: vec4<f32>, distance: f32, end: f32, scale: f32, background: vec3<f32>) -> vec4<f32> {
    let weight = clamp((end - abs(distance)) * scale, 0.0, 1.0);
    return vec4<f32>(mix(background, color.rgb, weight), color.a);
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return select(color / 12.92, pow((color + 0.055) / 1.055, vec3<f32>(2.4)), color > vec3<f32>(0.04045));
}
