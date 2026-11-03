// Shared metallic/roughness shading. Color maps decode through sRGB GPU views;
// all normal/scalar maps are linear. No source pixels are recolored or packed.
@group(1) @binding(0) var base_texture:texture_2d<f32>;
@group(1) @binding(1) var base_sampler:sampler;
@group(1) @binding(2) var normal_texture:texture_2d<f32>;
@group(1) @binding(3) var normal_sampler:sampler;
@group(1) @binding(4) var metal_texture:texture_2d<f32>;
@group(1) @binding(5) var metal_sampler:sampler;
@group(1) @binding(6) var rough_texture:texture_2d<f32>;
@group(1) @binding(7) var rough_sampler:sampler;
@group(1) @binding(8) var ao_texture:texture_2d<f32>;
@group(1) @binding(9) var ao_sampler:sampler;
@group(1) @binding(10) var emissive_texture:texture_2d<f32>;
@group(1) @binding(11) var emissive_sampler:sampler;
struct Material { color:vec4<f32>, properties:vec4<f32>, normal_ao:vec4<f32>, uv:vec4<f32>, metallic_channel:vec4<f32>, roughness_channel:vec4<f32>, occlusion_channel:vec4<f32>, emissive:vec4<f32> };
@group(1) @binding(12) var<uniform> material:Material;

@group(1) @binding(13) var sky_environment:texture_cube<f32>;
@group(1) @binding(14) var sky_sampler:sampler;
@group(1) @binding(15) var sky_brdf:texture_2d<f32>;

fn pbr_uv(uv:vec2<f32>)->vec2<f32> { return uv*material.uv.xy+material.uv.zw; }
fn pbr_base(uv:vec2<f32>)->vec4<f32> { return textureSample(base_texture,base_sampler,uv)*material.color; }
fn pbr_normal(position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, tangent:vec4<f32>)->vec3<f32> {
    let n=normalize(normal);
    let px=dpdx(position);let py=dpdy(position);let ux=dpdx(uv);let uy=dpdy(uv);
    // UV-derived cotangent frame handles procedural surfaces and mirrored UVs.
    let det=ux.x*uy.y-ux.y*uy.x;
    let raw_t=(px*uy.y-py*ux.y)*sign(det);
    let raw_b=(py*ux.x-px*uy.x)*sign(det);
    let projected_t=raw_t-n*dot(n,raw_t);
    var t=normalize(projected_t+vec3<f32>(1e-20));
    var b=normalize(cross(n,t))*select(-1.0,1.0,dot(cross(n,t),raw_b)>=0.0);
    if dot(tangent.xyz,tangent.xyz)>0.1 {
        t=normalize(tangent.xyz-n*dot(n,tangent.xyz))*sign(material.uv.x);
        b=cross(n,t)*tangent.w*sign(material.uv.x*material.uv.y);
    }
    let sample=textureSample(normal_texture,normal_sampler,uv).xyz*2.0-1.0;
    let mapped=normalize(vec3<f32>(sample.x*material.normal_ao.x,sample.y*material.normal_ao.x*material.normal_ao.y,sample.z));
    if material.normal_ao.x==0.0 || (dot(tangent.xyz,tangent.xyz)<0.1 && (abs(det)<1e-12 || dot(projected_t,projected_t)<1e-20)) { return n; }
    return normalize(t*mapped.x+b*mapped.y+n*mapped.z);
}
fn pbr_shade(position:vec3<f32>, n:vec3<f32>, eye:vec3<f32>, uv:vec2<f32>, color:vec3<f32>)->vec3<f32> {
    let metal=clamp(material.properties.x*dot(textureSample(metal_texture,metal_sampler,uv),material.metallic_channel),0.0,1.0);
    let r=dot(textureSample(rough_texture,rough_sampler,uv),material.roughness_channel);
    // The sole smoothness conversion. Uploads/mips contain original linear data;
    // genuine roughness maps set normal_ao.w to zero and pass through unchanged.
    let rough=clamp(material.properties.y*mix(r,1.0-r,material.normal_ao.w),0.045,1.0);
    let ao=mix(1.0,dot(textureSample(ao_texture,ao_sampler,uv),material.occlusion_channel),material.normal_ao.z);
    let l=normalize(vec3<f32>(0.35,0.85,0.4));let v=normalize(eye-position);let h=normalize(l+v);
    let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),0.001);let nh=max(dot(n,h),0.0);let vh=max(dot(v,h),0.0);
    let f0=mix(vec3<f32>(0.04),color,metal);
    let f=f0+(vec3<f32>(1.0)-f0)*pow(1.0-vh,5.0);
    let a2=pow(rough,4.0);let denom=nh*nh*(a2-1.0)+1.0;
    let distribution=a2/max(3.14159265*denom*denom,1e-7);
    // Height-correlated Smith GGX visibility.
    let visibility=0.5/max(nl*sqrt(nv*nv*(1.0-a2)+a2)+nv*sqrt(nl*nl*(1.0-a2)+a2),1e-5);
    let diffuse=(vec3<f32>(1.0)-f)*(1.0-metal)*color/3.14159265;
    let direct=(diffuse+f*distribution*visibility)*nl*2.0;
    // Distant procedural sky/ground only; no scene geometry reflections.
    // Mip levels contain a GGX convolution, not ordinary image minification.
    let reflection=reflect(-v,n);
    let radiance=textureSampleLevel(sky_environment,sky_sampler,reflection,rough*6.0).rgb;
    let integrated=textureSampleLevel(sky_brdf,sky_sampler,vec2<f32>(nv,rough),0.0).rg;
    let specular=radiance*(f0*integrated.x+vec3<f32>(integrated.y));
    // Broadest sky convolution approximates diffuse irradiance/pi.
    let irradiance=textureSampleLevel(sky_environment,sky_sampler,n,6.0).rgb;
    let ambient=((vec3<f32>(1.0)-f0)*(1.0-metal)*color*irradiance+specular)*ao;
    let emissive=textureSample(emissive_texture,emissive_sampler,uv).rgb*material.emissive.rgb;
    // Local Reinhard shoulder before the sRGB render target conversion; UI
    // colors are composited later and never pass through this tone mapper.
    let hdr=direct+ambient+emissive;
    return hdr/(vec3<f32>(1.0)+hdr);
}
