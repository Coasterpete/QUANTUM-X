@group(0) @binding(0) var mask:texture_2d<f32>;
@group(0) @binding(1) var scene_depth:texture_depth_2d;

@vertex fn vertex(@builtin(vertex_index) index:u32)->@builtin(position) vec4<f32> {
    let positions=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));
    return vec4<f32>(positions[index],0.0,1.0);
}

@fragment fn fragment(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let p=vec2<i32>(position.xy);
    // Only background pixels can receive an outline. Even another selected or
    // unselected surface (including a GLB train) retains its exact shaded color.
    if textureLoad(scene_depth,p,0)<1.0 { discard; }
    let size=vec2<i32>(textureDimensions(mask));
    var inner=false;
    var outer=false;
    for (var y=-2;y<=2;y++) {
        for (var x=-2;x<=2;x++) {
            let q=p+vec2<i32>(x,y);
            if all(q>=vec2<i32>(0)) && all(q<size) && x*x+y*y<=5 {
                let hit=textureLoad(mask,q,0).r>0.5;
                inner=inner || (hit && abs(x)<=1 && abs(y)<=1);
                outer=outer || hit;
            }
        }
    }
    if !outer { discard; }
    // A one-pixel gold core with a dark outer rim remains legible on both light
    // and dark backgrounds without broad wire coverage over the user's finish.
    return vec4<f32>(select(vec3<f32>(0.012),vec3<f32>(0.9,0.65,0.3),inner),1.0);
}
