struct Camera { matrix:mat4x4<f32>, selection:vec4<f32>, eye:vec4<f32> };
@group(0) @binding(0) var<uniform> camera:Camera;
struct Out { @builtin(position) clip:vec4<f32>, @location(0) normal:vec3<f32>, @location(1) color:vec3<f32>, @location(2) properties:vec3<f32>, @location(3) position:vec3<f32>, @location(4) uv:vec2<f32> };
@vertex fn vertex(@location(0) p:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) station:f32,
@location(4) t:vec3<f32>,@location(5) n:vec3<f32>,@location(6) l:vec3<f32>,@location(7) translation:vec3<f32>,@location(8) color:vec3<f32>,@location(9) props:vec3<f32>)->Out {
    let basis=mat3x3<f32>(t,n,l); var out:Out;
    out.position=basis*p+translation; out.clip=camera.matrix*vec4<f32>(out.position,1.0);out.normal=basis*normal;out.color=color;
    out.properties=vec3<f32>(props.xy,station+props.z);out.uv=uv;return out;
}
fn base(in:Out)->vec3<f32> {
    let selected=camera.selection.z>0.5 && in.properties.z>=camera.selection.x && in.properties.z<=camera.selection.y;
    return select(in.color,vec3<f32>(0.9,0.76,0.4),selected);
}
@fragment fn fragment(in:Out)->@location(0) vec4<f32> {
    let uv=pbr_uv(in.uv);
    let n=pbr_normal(in.position,in.normal,uv,vec4<f32>(0.0));
    let color=pbr_base(uv).rgb;
    return vec4<f32>(pbr_shade(in.position,n,camera.eye.xyz,uv,color),1.0);
}
// Selection is a separate visibility mask, never an input to PBR shading.
@fragment fn selection_mask(in:Out)->@location(0) vec4<f32> {
    if camera.selection.z<0.5 || in.properties.z<camera.selection.x || in.properties.z>camera.selection.y { discard; }
    return vec4<f32>(1.0);
}
@fragment fn wire(in:Out)->@location(0) vec4<f32> { return vec4<f32>(base(in)*0.6+vec3<f32>(0.3),1.0); }
