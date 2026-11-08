// A cut's style look over its picture (`style.rs`, line for line), drawn
// by the transition pass on one picture (`from_pic`), `p` the clip's time.
// Appended to `transition.wgsl`, whose `t`, `from_c`, `hash`, `tear` and
// `disc` these use. Ours, not ports.

fn luma(c: vec3f) -> f32 {
    return dot(c, vec3f(0.2126, 0.7152, 0.0722));
}

fn old_film(t_: f32, uv: vec2f, pixel: vec2u) -> vec3f {
    let amount = t.ka.x;
    let grain = t.ka.y;
    let scratches = t.ka.z;
    let flicker = t.ka.w;
    let fade = t.kb.x;
    let weave = t.kb.y;
    let frame = u32(floor(t_ * 24.0));
    let wob = vec2f((hash(vec2u(frame, 0u), 20u) - 0.5) * 0.006 * weave, (hash(vec2u(frame, 1u), 20u) - 0.5) * 0.004 * weave);
    let c = from_c(uv);
    let moved = from_c(clamp(uv + wob, vec2f(0.0), vec2f(1.0)));
    let grey = luma(moved);
    var o = mix(moved, vec3f(grey * 1.07 + 0.03, grey * 0.95 + 0.02, grey * 0.75 + 0.01), fade);
    let r2 = (uv.x - 0.5) * (uv.x - 0.5) + (uv.y - 0.5) * (uv.y - 0.5);
    o = o * ((1.0 + (hash(vec2u(frame, 2u), 20u) - 0.5) * 0.16 * flicker) * (1.0 - 0.7 * r2));
    for (var k = 0u; k < 3u; k++) {
        let x = hash(vec2u(frame, k + 3u), 21u);
        if hash(vec2u(frame, k + 3u), 22u) < scratches * 0.8 && abs(uv.x - x) < 0.0012 {
            o = mix(o, vec3f(0.9, 0.88, 0.8), 0.6);
        }
    }
    if hash(pixel / 3u, 23u + frame) > 1.0 - 0.002 * scratches {
        o = o * 0.3;
    }
    let g = (hash(pixel, 30u + frame) - 0.5) * 0.18 * grain;
    return mix(c, o + g, amount);
}

fn vhs(t_: f32, uv: vec2f, pixel: vec2u) -> vec3f {
    let amount = t.ka.x;
    let bleed = t.ka.y;
    let tracking = t.ka.z;
    let noise = t.ka.w;
    let frame = u32(floor(t_ * 30.0));
    let row = u32(floor(uv.y * 240.0));
    let in_band = select(0.0, 1.0, abs(uv.y - fract(t_ * 0.13)) < 0.04);
    let x = uv.x + in_band * (hash(vec2u(row, frame), 24u) - 0.5) * 0.03 * tracking + (hash(vec2u(row, frame), 25u) - 0.5) * 0.002 * noise;
    let c = from_c(uv);
    let sharp = from_c(clamp(vec2f(x, uv.y), vec2f(0.0), vec2f(1.0)));
    var soft = vec3f(0.0);
    for (var i = 0; i < 5; i++) {
        soft += from_c(clamp(vec2f(x + (f32(i) - 2.0) * 0.003 * bleed + 0.004 * bleed, uv.y), vec2f(0.0), vec2f(1.0))) / 5.0;
    }
    var o = soft + (luma(sharp) - luma(soft));
    o = mix(o, vec3f(luma(o)), 0.15) * 0.9 + 0.05;
    if pixel.y % 2u == 1u {
        o = o * 0.92;
    }
    let n = (hash(pixel, 40u + frame) - 0.5) * 0.12 * noise * (1.0 + 2.0 * in_band * tracking);
    return mix(c, o + n, amount);
}

fn cyberpunk(t_: f32, uv: vec2f, pixel: vec2u) -> vec3f {
    let amount = t.ka.x;
    let neon = t.ka.y;
    let split = t.ka.z;
    let scanlines = t.ka.w;
    let glitch = t.kb.x;
    let frame = u32(floor(t_ * 24.0));
    let c = from_c(uv);
    let d = uv - 0.5;
    let r = from_c(clamp(0.5 + d * (1.0 + 0.012 * split), vec2f(0.0), vec2f(1.0)));
    let b = from_c(clamp(0.5 + d * (1.0 - 0.012 * split), vec2f(0.0), vec2f(1.0)));
    var base = vec3f(r.r, c.g, b.b);
    var burst = 0.0;
    if hash(vec2u(frame / 6u, 0u), 26u) < 0.25 {
        burst = glitch;
    }
    if burst > 0.0 {
        base = tear(36.0, burst, 0.0, frame, uv, pixel, true);
    }
    let l = luma(base);
    let s = clamp((l - 0.2) / 0.6, 0.0, 1.0);
    let tint = mix(vec3f(0.1, 0.9, 1.0), vec3f(1.0, 0.3, 0.9), s * s * (3.0 - 2.0 * s));
    var o = ((base * (1.0 - 0.5 * neon) + tint * l * neon) - 0.5) * (1.0 + 0.3 * neon) + 0.5;
    if pixel.y % 3u == 0u {
        o = o * (1.0 - 0.3 * scanlines);
    }
    return mix(c, o, amount);
}

fn glow(uv: vec2f, aspect: f32, pixel: vec2u) -> vec3f {
    let c = from_c(uv);
    let bright = disc(0.005 + 0.04 * t.ka.y, 27u, uv, aspect, pixel, t.ka.z);
    return c + bright * t.ka.x * 2.0;
}

fn soft_focus(uv: vec2f, aspect: f32, pixel: vec2u) -> vec3f {
    let amount = t.ka.x;
    let c = from_c(uv);
    let b = disc(0.003 + 0.03 * t.ka.y, 28u, uv, aspect, pixel, -1.0);
    return c + (b - c) * amount * 0.7 + max(b - c, vec3f(0.0)) * amount * 0.3;
}
