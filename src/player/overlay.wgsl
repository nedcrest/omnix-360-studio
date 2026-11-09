// An overlay picture drawn over the stitched view: a turned quad, its
// picture premultiplied, graded, blurred and revealed as
// `overlay::composite` does it on the CPU.

struct Overlay {
    // The view's size and its origin in the target, pixels.
    frame: vec4f,
    // The target's size, 1 if it's sRGB, how far faded to black.
    dest: vec4f,
    // Centre on the view and half size, pixels.
    quad: vec4f,
    // cos and sin of the clockwise turn, opacity, 1 if graded.
    turn: vec4f,
    // `Grade::uniforms`, its opacity and fade left out (1 and 0).
    grade: array<vec4f, 5>,
    // The blur's reach across and down (`overlay::blur_reach`), the
    // reveal, 1 if `order` holds its letters' order.
    show: vec4f,
}

@group(0) @binding(0) var<uniform> o: Overlay;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var pic: texture_2d<f32>;
@group(0) @binding(3) var order: texture_2d<f32>;

// `overlay::BLUR_TAPS` and `overlay::WIPE_EDGE`.
const TAPS: u32 = 48u;
const WIPE_EDGE: f32 = 0.04;

// `Picture::blurred`: the picture averaged over a sunflower disc.
fn blurred(uv: vec2f) -> vec4f {
    if o.show.x <= 0.0 && o.show.y <= 0.0 {
        return textureSampleLevel(pic, samp, uv, 0.0);
    }
    var sum = vec4f(0.0);
    for (var i = 0u; i < TAPS; i++) {
        let r = sqrt((f32(i) + 0.5) / f32(TAPS));
        let a = f32(i) * 2.3999632;
        let at = clamp(uv + vec2f(cos(a), sin(a)) * r * o.show.xy, vec2f(0.0), vec2f(1.0));
        sum += textureSampleLevel(pic, samp, at, 0.0);
    }
    return sum / f32(TAPS);
}

// `overlay::revealed`: a picture wiped in from the left, text typed.
fn revealed(uv: vec2f) -> f32 {
    let reveal = o.show.z;
    if reveal >= 1.0 {
        return 1.0;
    }
    if o.show.w > 0.5 {
        let size = vec2f(textureDimensions(order));
        let at = min(vec2u(uv * size), vec2u(size) - vec2u(1u));
        return select(0.0, 1.0, textureLoad(order, at, 0).r <= reveal);
    }
    return clamp((reveal * (1.0 + WIPE_EDGE) - uv.x) / WIPE_EDGE, 0.0, 1.0);
}

struct Out {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn ovs(@builtin(vertex_index) i: u32) -> Out {
    // A triangle strip over the quad, y down as on screen.
    let corners = array(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0), vec2f(1.0, 1.0));
    let q = corners[i];
    let p = vec2f(q.x * o.quad.z, q.y * o.quad.w);
    // Turned clockwise (y down), as `Quad::to_item` undoes it.
    let r = vec2f(o.turn.x * p.x - o.turn.y * p.y, o.turn.y * p.x + o.turn.x * p.y);
    let px = o.frame.zw + o.quad.xy + r;
    var out: Out;
    out.pos = vec4f(px.x / o.dest.x * 2.0 - 1.0, 1.0 - px.y / o.dest.y * 2.0, 0.0, 1.0);
    out.uv = (q + vec2f(1.0)) / 2.0;
    return out;
}

@fragment
fn ofs(v: Out) -> @location(0) vec4f {
    let p = blurred(v.uv);
    let a = p.a * o.turn.z * revealed(v.uv);
    if a <= 0.0 {
        discard;
    }
    var c = p.rgb / max(p.a, 1e-6);
    if o.turn.w > 0.5 {
        c = grade_by(o.grade[0], o.grade[1], o.grade[2], o.grade[3], o.grade[4], c, v.uv * 2.0 - vec2f(1.0), o.quad.z / o.quad.w);
    }
    c = c * (1.0 - o.dest.w);
    if o.dest.z > 0.5 {
        c = select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045));
    }
    // Premultiplied, over what's drawn.
    return vec4f(c * a, a);
}
