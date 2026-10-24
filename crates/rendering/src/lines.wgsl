struct Camera { matrix: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
struct VertexOut { @builtin(position) clip: vec4<f32>, @location(0) color: vec3<f32> };
@vertex fn vertex(@location(0) position:vec3<f32>,@location(1) color:vec3<f32>)->VertexOut {
    var out:VertexOut;out.clip=camera.matrix*vec4<f32>(position,1.0);out.color=color;return out;
}
@fragment fn fragment(in:VertexOut)->@location(0) vec4<f32> {return vec4<f32>(in.color,1.0);}
