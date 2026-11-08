// A join's transition: the outgoing and the incoming cut, each drawn whole
// into a picture of its own, mixed here into the view. `transition.rs`'s
// `colour` is this on the CPU, line for line (the export's twin). The looks
// are ports of gl-transitions (MIT): WhipPan and WhipSpin by gre, CrossZoom
// by rectalogic, Overexposure by Ben Zhang, their parameters the knobs (`Look::knobs`);
// light leak after FilmBurn by Anastasia Dunbar, glitch after
// StripDatamoshGlitch by bread and GlitchMemories by Gunnar Roth, their
// sine noise put on `hash`; luma fade is ours.

struct T {
    // The look (`Look::code`), how far through, the view's width over its
    // height, 1 if the pictures are sRGB-encoded textures.
    look: vec4f,
    // The view's size and its top-left corner in the target, pixels.
    view: vec4f,
    // The look's knobs (`transition::Values`), in `Look::knobs` order.
    ka: vec4f,
    kb: vec4f,
}

@group(0) @binding(0) var<uniform> t: T;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var from_pic: texture_2d<f32>;
@group(0) @binding(3) var to_pic: texture_2d<f32>;

const PI: f32 = 3.141592653589793;
const SAMPLES: u32 = 24u;

@vertex
fn tvs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let xy = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(xy * 2.0 - 1.0, 0.0, 1.0);
}

// The value the CPU reads: sRGB-encoded, as the export's frames are.
fn enc(c: vec3f) -> vec3f {
    if t.look.w < 0.5 {
        return c;
    }
    return select(1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3f(0.0031308));
}

fn dec(c: vec3f) -> vec3f {
    if t.look.w < 0.5 {
        return c;
    }
    return select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045));
}

fn from_c(uv: vec2f) -> vec3f {
    return enc(textureSampleLevel(from_pic, samp, uv, 0.0).rgb);
}

fn to_c(uv: vec2f) -> vec3f {
    return enc(textureSampleLevel(to_pic, samp, uv, 0.0).rgb);
}

// `transition::hash`: integers only, the same on both.
fn hash(pixel: vec2u, salt: u32) -> f32 {
    var v = (pixel.x * 1973u + pixel.y * 9277u + salt * 26699u) | 1u;
    v ^= v << 13u;
    v ^= v >> 17u;
    v ^= v << 5u;
    return f32(v >> 8u) / 16777216.0;
}

fn expo_in_out(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let y = x * 2.0;
    if y < 1.0 {
        return 0.5 * pow(2.0, 10.0 * (y - 1.0));
    }
    return 0.5 * (2.0 - pow(2.0, -10.0 * (y - 1.0)));
}

fn mirror(v: f32) -> f32 {
    let m = v - 2.0 * floor(v / 2.0);
    return select(m, 2.0 - m, m > 1.0);
}

// The whip's move: left, right, up, down (`Look::knobs`' order).
fn whip_direction(i: f32) -> vec2f {
    switch u32(i) {
        case 1u: { return vec2f(1.0, 0.0); }
        case 2u: { return vec2f(0.0, -1.0); }
        case 3u: { return vec2f(0.0, 1.0); }
        default: { return vec2f(-1.0, 0.0); }
    }
}

fn sigmoid(x: f32, a: f32) -> f32 {
    if x > 0.5 {
        return 1.0 - pow(2.0 - 2.0 * x, a) / 2.0;
    }
    return pow(2.0 * x, a) / 2.0;
}

fn light_leak(p: f32, uv: vec2f, aspect: f32) -> vec3f {
    let glow = max(sin(PI * p), 0.0);
    let base = mix(from_c(uv), to_c(uv), sigmoid(p, 10.0));
    var f = 0.0;
    for (var i = 0u; i < 6u; i++) {
        let r = vec4f(hash(vec2u(i, 0u), 5u), hash(vec2u(i, 1u), 5u), hash(vec2u(i, 2u), 5u), hash(vec2u(i, 3u), 5u));
        let centre = vec2f(r.x * 1.4 - 0.2 + (r.y - 0.5) * p, r.z);
        let radius = 0.15 + 0.35 * r.w;
        let d = vec2f((uv.x - centre.x) * aspect, uv.y - centre.y);
        f += exp(-dot(d, d) / (radius * radius));
        f += 0.06 * (1.0 + sin(uv.x * 6.0 * r.y + p * 8.0 + r.z * 6.0) * sin(uv.y * 6.0 * r.w + p * 6.0 + r.x * 6.0));
    }
    let g = f / 2.2;
    let warm = vec3f(g, pow(g * 0.7, 2.0 - glow), pow(g * 0.6, 1.3));
    return base + warm * glow * t.ka.x;
}

fn glitch(p: f32, uv: vec2f, pixel: vec2u) -> vec3f {
    let burst = sqrt(max(sin(PI * p), 0.0)) * t.ka.x;
    let frame = u32(floor(p * 24.0));
    let row = u32(floor(uv.y * t.ka.y));
    let torn = hash(vec2u(row, frame), 7u) < 0.35 * burst;
    var shift = 0.0;
    if torn {
        shift = (hash(vec2u(row, frame), 8u) - 0.5) * 0.25 * burst;
    }
    var w = smoothstep(0.3, 0.7, p);
    if torn && hash(vec2u(row, frame), 9u) < 0.5 {
        w = 1.0 - w;
    }
    let split = select(0.004, 0.025, torn) * burst;
    let grain = (hash(pixel, 10u) - 0.5) * 0.12 * burst;
    let r = vec2f(clamp(uv.x + shift + split, 0.0, 1.0), uv.y);
    let g = vec2f(clamp(uv.x + shift, 0.0, 1.0), uv.y);
    let b = vec2f(clamp(uv.x + shift - split, 0.0, 1.0), uv.y);
    return vec3f(mix(from_c(r), to_c(r), w).r, mix(from_c(g), to_c(g), w).g, mix(from_c(b), to_c(b), w).b) + grain;
}

fn transition(code: u32, p: f32, uv: vec2f, aspect: f32, pixel: vec2u) -> vec3f {
    switch code {
        case 1u: {
            let d = whip_direction(t.ka.x);
            let h = 1.0 - abs(2.0 * p - 1.0);
            let e = 0.5 * h * (0.1 + 0.9 * h * h);
            let pos = select(1.0 - e, e, p < 0.5);
            let len = 0.4 * h * (0.1 + 2.7 * h * h) / 2.8 * t.ka.y;
            let q = uv - d * pos;
            let jitter = hash(pixel, 1u);
            var sum = vec3f(0.0);
            for (var i = 0u; i < SAMPLES; i++) {
                let k = (f32(i) + jitter) / f32(SAMPLES) - 0.5;
                let s = q + d * k * len;
                if dot(s - 0.5, -d) < 0.5 * dot(d, d) {
                    sum += from_c(clamp(s, vec2f(0.0), vec2f(1.0)));
                } else {
                    sum += to_c(clamp(s + d, vec2f(0.0), vec2f(1.0)));
                }
            }
            return sum / f32(SAMPLES);
        }
        case 2u: {
            var centre = vec2f(0.5, 0.5);
            if t.ka.y < 0.5 {
                centre = vec2f(0.25 + 0.5 * p, 0.5);
            }
            let dissolve = expo_in_out(p);
            let s = t.ka.x / 2.0 * (1.0 - cos(2.0 * PI * p));
            let to_centre = centre - uv;
            let offset = hash(pixel, 2u);
            var sum = vec3f(0.0);
            var total = 0.0;
            for (var i = 0u; i <= 40u; i++) {
                let percent = (f32(i) + offset) / 40.0;
                let weight = 4.0 * (percent - percent * percent);
                let at = clamp(uv + to_centre * percent * s, vec2f(0.0), vec2f(1.0));
                sum += mix(from_c(at), to_c(at), dissolve) * weight;
                total += weight;
            }
            return sum / total;
        }
        case 3u: {
            let h = 1.0 - abs(2.0 * p - 1.0);
            let shutter = t.ka.z * 0.1 * h;
            let scale = 1.0 + t.kb.x * h * h * h;
            let q = vec2f((uv.x - 0.5) * aspect, uv.y - 0.5);
            let jitter = hash(pixel, 3u);
            var sum = vec3f(0.0);
            var weight = vec3f(0.0);
            for (var i = 0u; i < SAMPLES; i++) {
                let s = (f32(i) + jitter) / f32(SAMPLES) - 0.5;
                let tt = clamp(p + s * shutter, 0.0, 1.0);
                let half = PI * t.ka.x * (1.0 - 2.0 * t.ka.y);
                var a = -half * pow(2.0 - 2.0 * tt, 3.0);
                if tt < 0.5 {
                    a = half * pow(2.0 * tt, 3.0);
                }
                let c = cos(a);
                let sn = sin(a);
                let r = vec2f(c * q.x + sn * q.y, -sn * q.x + c * q.y) / scale;
                let suv = vec2f(mirror(r.x / aspect + 0.5), mirror(r.y + 0.5));
                var col = to_c(suv);
                if tt < 0.5 {
                    col = from_c(suv);
                }
                let w = max(vec3f(1.0 - 4.0 * t.ka.w * s, 1.0, 1.0 + 4.0 * t.ka.w * s), vec3f(0.0));
                sum += col * w;
                weight += w;
            }
            return sum / max(weight, vec3f(1e-5));
        }
        case 4u: {
            let lift = sin(PI * p) * t.ka.x;
            let tint = mix(vec3f(1.0), vec3f(1.0, 0.8, 0.55), t.ka.y);
            let a = from_c(uv);
            let b = to_c(uv);
            return a * (1.0 - p) + b * p + (a + b) * lift * tint;
        }
        case 5u: {
            return light_leak(p, uv, aspect);
        }
        case 6u: {
            let a = from_c(uv);
            let soft = t.ka.x;
            var luma = dot(a, vec3f(0.2126, 0.7152, 0.0722));
            if t.ka.y >= 0.5 {
                luma = 1.0 - luma;
            }
            let w = clamp((p * (1.0 + soft) - (1.0 - luma)) / soft, 0.0, 1.0);
            return mix(a, to_c(uv), w * w * (3.0 - 2.0 * w));
        }
        case 7u: {
            return glitch(p, uv, pixel);
        }
        default: {
            return mix(from_c(uv), to_c(uv), p);
        }
    }
}

@fragment
fn tfs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let local = pos.xy - t.view.zw;
    let uv = local / t.view.xy;
    let c = transition(u32(t.look.x), t.look.y, uv, t.look.z, vec2u(local));
    return vec4f(dec(clamp(c, vec3f(0.0), vec3f(1.0))), 1.0);
}
