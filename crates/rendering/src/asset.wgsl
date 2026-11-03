struct Camera { matrix:mat4x4<f32>, eye:vec4<f32> };
@group(0) @binding(0) var<uniform> camera:Camera;
struct Out { @builtin(position) clip:vec4<f32>, @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) @interpolate(flat) sign:f32, @location(4) tangent:vec4<f32> };
@vertex fn vertex(@location(0) p:vec3<f32>,@location(1) n:vec3<f32>,@location(2) uv:vec2<f32>,@location(10) tangent:vec4<f32>,@location(3) a:vec4<f32>,@location(4) b:vec4<f32>,@location(5) c:vec4<f32>,@location(6) d:vec4<f32>,@location(7) nx:vec4<f32>,@location(8) ny:vec4<f32>,@location(9) nz:vec4<f32>)->Out {
    var out:Out;let world=mat4x4<f32>(a,b,c,d)*vec4<f32>(p,1.0);out.position=world.xyz;out.clip=camera.matrix*world;out.normal=mat3x3<f32>(nx.xyz,ny.xyz,nz.xyz)*n;out.uv=uv;out.sign=nx.w;out.tangent=vec4<f32>(mat3x3<f32>(a.xyz,b.xyz,c.xyz)*tangent.xyz,tangent.w*nx.w);return out;
}
@fragment fn fragment(in:Out,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
    let facing=select(!front,front,in.sign>0.0);
    let uv=pbr_uv(in.uv);
    let base=pbr_base(uv);
    let n=pbr_normal(in.position,in.normal*select(-1.0,1.0,facing),uv,in.tangent);
    let color=pbr_shade(in.position,n,camera.eye.xyz,uv,base.rgb);
    if !facing && material.properties.w<0.5 { discard; }
    if material.properties.z>=0.0 && base.a<material.properties.z { discard; }
    return vec4<f32>(color,1.0);
}
