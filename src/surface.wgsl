struct Scene {
    camera: mat4x4<f32>,
    fog_color: vec4<f32>,
    fog_distances: vec4<f32>,
}
@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var surface_texture: texture_2d<f32>;
@group(0) @binding(2) var surface_sampler: sampler;
@group(0) @binding(3) var secondary_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) specular: vec3<f32>,
    @location(4) secondary_uv: vec2<f32>,
    @location(5) diffuse_alpha: f32,
    @location(6) eye_depth: f32,
}

@vertex
fn vertex_main(
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) specular: vec3<f32>,
    @location(4) secondary_uv: vec2<f32>,
    @location(5) diffuse_alpha: f32,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip = scene.camera * vec4(position, 1.0);
    out.eye_depth = out.clip.w;
    out.world = position;
    out.color = color;
    out.uv = uv;
    out.specular = specular;
    out.secondary_uv = secondary_uv;
    out.diffuse_alpha = diffuse_alpha;
    return out;
}

fn apply_fog(color: vec4<f32>, depth: f32) -> vec4<f32> {
    if scene.fog_color.a == 0.0 {
        return color;
    }
    // Perspective interpolation of vertex clip w gives eye depth at the pixel.
    // An explicit varying avoids this host's divergent fragment position.w.
    // Original WFOG fogs RGB after texture/specular stages, before blending.
    let factor = clamp((scene.fog_distances.y - depth) /
        (scene.fog_distances.y - scene.fog_distances.x), 0.0, 1.0);
    return vec4(mix(scene.fog_color.rgb, color.rgb, factor), color.a);
}

@fragment
fn fragment_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(cross(dpdx(in.world), dpdy(in.world)));
    let light = 0.4 + 0.6 * abs(dot(normal, normalize(vec3(0.4, 1.0, 0.6))));
    let texel = textureSample(surface_texture, surface_sampler, in.uv);
    // Solid template diagnostics use sampled RGB with opaque coverage.
    return vec4(in.color * texel.rgb * light, 1.0);
}


@fragment
fn water_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // State 9 modulates texture by original constant ARGB 0xfffffffe.
    // Its stage0 uses the same -0.5 mip bias as the static scenery state.
    let texel = textureSampleBias(surface_texture, surface_sampler, in.uv, -0.5);
    return apply_fog(texel * vec4(1.0, 1.0, 254.0 / 255.0, 1.0), in.eye_depth);
}


@fragment
fn water_fallback_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(surface_texture, surface_sampler, in.uv);
    return apply_fog(vec4(in.color * texel.rgb, 0.0), in.eye_depth);
}

@fragment
fn static_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSampleBias(surface_texture, surface_sampler, in.uv, -0.5);
    return apply_fog(vec4(clamp(in.color * texel.rgb + in.specular, vec3(0.0), vec3(1.0)), texel.a * in.diffuse_alpha), in.eye_depth);
}

@fragment
fn facade_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSampleBias(surface_texture, surface_sampler, in.uv, -0.5);
    let secondary = textureSample(secondary_texture, surface_sampler, in.secondary_uv);
    let current = clamp(in.color * texel.rgb, vec3(0.0), vec3(1.0));
    let alpha = texel.a * in.diffuse_alpha;
    // D3DTOP_BLENDCURRENTALPHA, TEXTURE / CURRENT; specular is added last.
    let blended = secondary.rgb * alpha + current * (1.0 - alpha);
    return apply_fog(vec4(clamp(blended + in.specular, vec3(0.0), vec3(1.0)), 1.0), in.eye_depth);
}

@fragment
fn cloud_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Original state18 modulates texture28 by diffuse, then texture27 by current.
    // Both color and alpha follow MODULATE; device blending is ONE / ONE.
    let first = textureSample(surface_texture, surface_sampler, in.uv);
    let second = textureSample(secondary_texture, surface_sampler, in.secondary_uv);
    return apply_fog(first * second * vec4(in.color, in.diffuse_alpha), in.eye_depth);
}

@fragment
fn star_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // A white single texel represents the original null texture binding.
    let texel = textureSample(surface_texture, surface_sampler, in.uv);
    return apply_fog(vec4(in.color * texel.rgb, in.diffuse_alpha * texel.a), in.eye_depth);
}
