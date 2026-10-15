struct Screen {
    size: vec2<f32>,
    _pad: vec2<f32>,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) radius: f32,
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(1) @binding(0) var<uniform> screen: Screen;

@vertex
fn vs(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) local: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) radius: f32,
) -> VsOut {
    let x = pos.x / screen.size.x * 2.0 - 1.0;
    let y = 1.0 - pos.y / screen.size.y * 2.0;
    var out: VsOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = uv;
    out.local = local;
    out.size = size;
    out.radius = radius;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    if (in.radius > 0.0) {
        let r = min(in.radius, min(in.size.x, in.size.y) * 0.5);
        let half = in.size * 0.5;
        let q = abs(in.local - half) - (half - vec2<f32>(r));
        let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
        if (d > 0.0) {
            discard;
        }
    }
    return textureSample(tex, tex_sampler, in.uv);
}
