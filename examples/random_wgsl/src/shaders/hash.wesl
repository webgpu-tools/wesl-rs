// On generating random numbers, with help of y= [(a+x)sin(bx)] mod 1", W.J.J. Rey, 22nd European Meeting of Statisticians 1998
// from https://gist.github.com/munrocket/236ed5ba7e409b8bdf1ff6eca5dcdc39
public fn rand11(n: f32) -> f32 {
    return fract(sin(n) * 43758.5453123);
}
public fn rand22(n: vec2f) -> f32 {
    return fract(sin(dot(n, vec2f(12.9898, 4.1414))) * 43758.5453);
}

// MIT License. © Stefan Gustavson, Munrocket
// https://gist.github.com/munrocket/236ed5ba7e409b8bdf1ff6eca5dcdc39
public fn permute4(x: vec4f) -> vec4f {
    return ((x * 34. + 1.) * x) % vec4f(289.);
}
public fn fade2(t: vec2f) -> vec2f {
    return t * t * t * (t * (t * 6. - 15.) + 10.);
}

