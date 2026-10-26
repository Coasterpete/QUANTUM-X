struct Camera { matrix:mat4x4<f32>, selection:vec4<f32>, eye:vec4<f32> };
@group(0) @binding(0) var<uniform> camera:Camera;
struct Out { @builtin(position) clip:vec4<f32>, @location(0) normal:vec3<f32>, @location(1) color:vec3<f32>, @location(2) properties:vec3<f32>, @location(3) position:vec3<f32> };
@vertex fn vertex(@location(0) p:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) station:f32,
@location(4) t:vec3<f32>,@location(5) n:vec3<f32>,@location(6) l:vec3<f32>,@location(7) translation:vec3<f32>,@location(8) color:vec3<f32>,@location(9) props:vec3<f32>)->Out {
    let basis=mat3x3<f32>(t,n,l); var out:Out;
    out.position=basis*p+translation; out.clip=camera.matrix*vec4<f32>(out.position,1.0);out.normal=basis*normal;out.color=color;
    out.properties=vec3<f32>(props.xy,station+props.z);return out;
}
fn base(in:Out)->vec3<f32> {
    let selected=camera.selection.z>0.5 && in.properties.z>=camera.selection.x && in.properties.z<=camera.selection.y;
    return select(in.color,mix(in.color,vec3<f32>(0.9,0.76,0.4),0.32),selected);
}
@fragment fn fragment(in:Out)->@location(0) vec4<f32> {
    let n=normalize(in.normal);let light=normalize(vec3<f32>(0.35,0.85,0.4));
    let view=normalize(camera.eye.xyz-in.position);let half=normalize(light+view);
    let metallic=in.properties.x;let roughness=clamp(in.properties.y,0.05,1.0);
    let color=base(in);let diffuse=max(dot(n,light),0.0);let fresnel=mix(vec3<f32>(0.04),color,metallic);
    let spec=pow(max(dot(n,half),0.0),mix(128.0,4.0,roughness))*(1.0-roughness*0.7);
    // Directional + hemispheric ambient, explicitly an initial material model.
    let ambient=0.24+0.1*(n.y*0.5+0.5);
    return vec4<f32>(color*(ambient+diffuse*0.65)+fresnel*spec,1.0);
}
@fragment fn wire(in:Out)->@location(0) vec4<f32> { return vec4<f32>(base(in)*0.6+vec3<f32>(0.3),1.0); }
