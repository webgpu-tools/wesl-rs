
fn rotate_3u(input: vec3u) -> vec3u {
    return vec3u(input.z, input.x, input.y);
}

struct Particle {
  pos : vec2f,
  vel : vec2f,
}
struct Particles {
  particles : array<Particle>,
}