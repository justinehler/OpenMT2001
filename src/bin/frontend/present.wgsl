@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Output { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vertex(@builtin(vertex_index) index: u32) -> Output {
    let uv = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Output;
    out.position = vec4f(uv * vec2f(2., -2.) + vec2f(-1., 1.), 0., 1.);
    out.uv = uv; return out;
}
@fragment fn fragment(in: Output) -> @location(0) vec4f { return vec4f(textureSample(image, image_sampler, in.uv).rgb, 1.); }
