struct uniforms {
    size: vec2<u32>,
    time: f32
};

struct audio_uniform {
    frequency: f32,
    magnitude: f32
};

const AUDIO_RANGE: u32 = 8u;

@group(0) @binding(0) var<uniform> data: uniforms;
@group(0) @binding(1) var<storage, read> audio_data: array<audio_uniform, AUDIO_RANGE>;


@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> @builtin(position) vec4<f32> {
    let x = f32(data.size.x) / 2.0;
    let y = f32(data.size.y) / 2.0;

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
    // let dist = sqrt(pow(center.x - frag_coords.x, 2.0) + pow(center.y - frag_coords.y, 2.0));
    
    let bar_scale = f32(data.size.x) / f32(AUDIO_RANGE);

    for (var i: u32 = 0u; i < AUDIO_RANGE; i = i + 1u) {
        let bar_x = f32(i) * bar_scale;
        let bar_y = audio_data[i].magnitude * f32(data.size.y);

        if (frag_coords.x >= bar_x && frag_coords.x <= bar_x + bar_scale && frag_coords.y >= bar_y) {
            return vec4<f32>(1.0, 1.0, 1.0, 1.0);
        }
    }



    let c = vec3<f32>(0.0,0.0,0.0);

    // if (dist < min(f32(data.size.x) / 2.0, f32(data.size.y) / 2.0) - 50.0) {
    //     return vec4<f32>(c, 1.0);
    // }
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}