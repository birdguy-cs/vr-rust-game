struct UniformData {
    matrices: array<mat4x4<f32>, 2>,
    model: mat4x4<f32>,
    uv_scale: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

@group(0) @binding(0) var<uniform> u_data: UniformData;
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @builtin(view_index) view_index: u32,
) -> VertexOutput {
    let matrix = u_data.matrices[view_index];

    var output: VertexOutput;

    let world_position = u_data.model * vec4<f32>(position, 1.0);

    output.position = matrix * world_position;
    output.normal = normalize((u_data.model * vec4<f32>(normal, 0.0)).xyz);
    output.uv = uv * u_data.uv_scale.xy;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let light_direction = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let normal = normalize(input.normal);

    let brightness = max(dot(normal, light_direction), 0.0);
    let ambient = 0.2;
    let lighting = ambient + brightness * 0.8;

    let base_color = textureSample(t_diffuse, s_diffuse, input.uv).rgb * vec3<f32>(0.8);

    return vec4<f32>(base_color * lighting, 1.0);
}
