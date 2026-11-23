struct uniforms {
    size: vec2<u32>,
    volume: u32,
    time: f32
};

@group(0) @binding(0) var<uniform> data: uniforms;


@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> @builtin(position) vec4<f32> {
    let x = f32(data.size.x) / 2.0;
    let y = f32(data.size.y) / 2.0;
    let volume = data.volume;

    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-x, -y),
        vec2<f32>( x, -y),
        vec2<f32>(-x,  y),
        vec2<f32>(-x,  y),
        vec2<f32>( x, -y),
        vec2<f32>( x,  y),
    );

    let p = positions[in_vertex_index];
    return vec4<f32>(p, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag_coords: vec4<f32>) -> @location(0) vec4<f32> {
    let center = vec2<f32>(data.size.xy) / 2.0;
    let dist = sqrt(pow(center.x - frag_coords.x, 2.0) + pow(center.y - frag_coords.y, 2.0));
    let c = vec3<f32>(f32(data.volume) / 4294967295, f32(data.volume) / 4294967295, f32(data.volume) / 4294967295);

    if (dist < min(f32(data.size.x) / 2.0, f32(data.size.y) / 2.0) - 50.0) {
        return vec4<f32>(c, 0.75);
    } else {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

}