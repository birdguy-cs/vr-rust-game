struct UniformData {
    matrices: array<mat4x4<f32>, 2>,
    model: mat4x4<f32>,
}

@group(0)
@binding(0)
var<uniform> u_data: UniformData;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @builtin(view_index) view_index: u32,
) -> @builtin(position) vec4<f32> {
    let matrix = u_data.matrices[view_index];

    let local_position = vec4<f32>(position, 1.0);

    return matrix * u_data.model * local_position;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
