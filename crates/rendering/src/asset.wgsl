struct Camera { matrix:mat4x4<f32>, eye:vec4<f32> };
@group(0) @binding(0) var<uniform> camera:Camera;
@group(1) @binding(0) var base_texture:texture_2d<f32>;
@group(1) @binding(1) var base_sampler:sampler;
struct Material { color:vec4<f32>, properties:vec4<f32>, double_sided:vec4<f32> };
@group(1) @binding(2) var<uniform> material:Material;
struct Out { @builtin(position) clip:vec4<f32>, @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) @interpolate(flat) sign:f32 };
@vertex fn vertex(@location(0) p:vec3<f32>,@location(1) n:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) a:vec4<f32>,@location(4) b:vec4<f32>,@location(5) c:vec4<f32>,@location(6) d:vec4<f32>,@location(7) nx:vec4<f32>,@location(8) ny:vec4<f32>,@location(9) nz:vec4<f32>)->Out {
    var out:Out;let world=mat4x4<f32>(a,b,c,d)*vec4<f32>(p,1.0);out.position=world.xyz;out.clip=camera.matrix*world;out.normal=mat3x3<f32>(nx.xyz,ny.xyz,nz.xyz)*n;out.uv=uv;out.sign=nx.w;return out;
}
@fragment fn fragment(in:Out,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
    let facing=select(!front,front,in.sign>0.0);
    if !facing && material.double_sided.x<0.5 { discard; }
    let base=textureSample(base_texture,base_sampler,in.uv)*material.color;
    if material.properties.z>=0.0 && base.a<material.properties.z { discard; }
    let n=normalize(in.normal)*select(-1.0,1.0,facing);
    let light=normalize(vec3<f32>(0.35,0.85,0.4));let view=normalize(camera.eye.xyz-in.position);let half=normalize(light+view);
    let metal=material.properties.x;let rough=clamp(material.properties.y,0.05,1.0);
    let spec=pow(max(dot(n,half),0.0),mix(128.0,4.0,rough))*(1.0-rough*0.7);
    return vec4<f32>(base.rgb*(0.28+0.08*n.y+max(dot(n,light),0.0)*0.65)+mix(vec3<f32>(0.04),base.rgb,metal)*spec,1.0);
}
