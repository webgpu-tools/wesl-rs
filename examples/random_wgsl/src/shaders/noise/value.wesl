import super::super::hash::rand11;
import super::super::hash::rand22;

// WTFPL License
// https://gist.github.com/munrocket/236ed5ba7e409b8bdf1ff6eca5dcdc39

public fn value_noise(p: f32) -> f32 {
    let fl = floor(p);
    return mix(rand11(fl), rand11(fl + 1.), fract(p));
}
public fn value_noise2(n: vec2f) -> f32 {
    let d = vec2f(0., 1.);
    let b = floor(n);
    let f = smoothstep(vec2f(0.), vec2f(1.), fract(n));
    return mix(mix(rand22(b), rand22(b + d.yx), f.x), mix(rand22(b + d.xy), rand22(b + d.yy), f.x), f.y);
}
