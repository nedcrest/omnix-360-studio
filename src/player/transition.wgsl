// A join's transition: the outgoing and the incoming cut, each drawn whole
// into a picture of its own, mixed here into the view. `transition.rs`'s
// `colour` is this on the CPU, line for line (the export's twin). The looks
// are ports of gl-transitions (MIT): WhipPan and WhipSpin by gre, CrossZoom
// by rectalogic, Overexposure by Ben Zhang, at their default parameters.

struct T {
    // The look (`Look::code`), how far through, the view's width over its
    // height, 1 if the pictures are sRGB-encoded textures.
    look: vec4f,
    // The view's size and its top-left corner in the target, pixels.
    view: vec4f,
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

fn transition(code: u32, p: f32, uv: vec2f, aspect: f32, pixel: vec2u) -> vec3f {
    switch code {
        case 1u: {
            let d = vec2f(-1.0, 0.0);
            let h = 1.0 - abs(2.0 * p - 1.0);
            let e = 0.5 * h * (0.1 + 0.9 * h * h);
            let pos = select(1.0 - e, e, p < 0.5);
            let len = 0.4 * h * (0.1 + 2.7 * h * h) / 2.8;
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
            let centre = vec2f(0.25 + 0.5 * p, 0.5);
            let dissolve = expo_in_out(p);
            let s = 0.4 / 2.0 * (1.0 - cos(2.0 * PI * p));
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
            let shutter = 0.5 * 0.1 * h;
            let scale = 1.0 + 0.25 * h * h * h;
            let q = vec2f((uv.x - 0.5) * aspect, uv.y - 0.5);
            let jitter = hash(pixel, 3u);
            var sum = vec3f(0.0);
            var weight = vec3f(0.0);
            for (var i = 0u; i < SAMPLES; i++) {
                let s = (f32(i) + jitter) / f32(SAMPLES) - 0.5;
                let tt = clamp(p + s * shutter, 0.0, 1.0);
                let half = PI;
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
                let w = max(vec3f(1.0 - 4.0 * 0.6 * s, 1.0, 1.0 + 4.0 * 0.6 * s), vec3f(0.0));
                sum += col * w;
                weight += w;
            }
            return sum / max(weight, vec3f(1e-5));
        }
        case 4u: {
            let lift = sin(PI * p) * 0.6;
            return from_c(uv) * (1.0 - p + lift) + to_c(uv) * (p + lift);
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
