struct Screen {
    size: vec2<f32>,
    _pad: vec2<f32>,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) params: vec2<f32>,
}

@group(0) @binding(0) var<uniform> screen: Screen;

@vertex
fn vs(
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) rect: vec4<f32>,
    @location(3) params: vec2<f32>,
) -> VsOut {
    let x = pos.x / screen.size.x * 2.0 - 1.0;
    let y = 1.0 - pos.y / screen.size.y * 2.0;
    var out: VsOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.color = color;
    out.local = pos - rect.xy;
    out.size = rect.zw;
    out.params = params;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let radius = in.params.x;
    let mode = in.params.y;
    var cover = 1.0;
    if (radius > 0.0) {
        let r = min(radius, min(in.size.x, in.size.y) * 0.5);
        let half = in.size * 0.5;
        let q = abs(in.local - half) - (half - vec2<f32>(r));
        let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
        let aa = max(fwidth(d), 0.75);
        cover = 1.0 - smoothstep(-aa, aa, d);
        if (cover <= 0.0) {
            discard;
        }
    }

    var alpha = in.color.a;
    if (mode > 0.5) {
        // Same stops as the page: dark at the bottom, clear a little above the middle.
        let g = in.local.y / max(in.size.y, 1.0);
        if (g < 0.48) {
            alpha = 0.0;
        } else if (g < 0.70) {
            let u = (g - 0.48) / 0.22;
            alpha = mix(0.0, 0.20, u);
        } else {
            let u = (g - 0.70) / 0.30;
            alpha = mix(0.20, 0.88, u);
        }
    }
    return vec4<f32>(in.color.rgb, alpha * cover);
}
