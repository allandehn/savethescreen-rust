#version 450
layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Globals {
    vec2 u_res;
    float u_time;
    float _pad;
};

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float fbm(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 4; i++) {
    v += a * noise(p);
    p = p * 2.02 + vec2(3.1, 1.4);
    a *= 0.5;
  }
  return v;
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_res.xy;
  float aspect = u_res.x / u_res.y;
  vec2 p = vec2(uv.x * aspect, uv.y) * 1.15;
  float t = u_time * 0.03;

  p += vec2(t * 0.12, -t * 0.08);

  vec2 q = p;
  q += 0.85 * vec2(fbm(q + t), fbm(q + vec2(4.1, 1.7) - t));
  q += 0.35 * vec2(noise(q * 1.5 - t * 0.5), noise(q * 1.4 + t * 0.4));

  float n = fbm(q * 1.2);
  float n2 = noise(q * 1.8 + 6.0);
  float ridges = pow(1.0 - abs(n * 2.0 - 1.0), 2.2);

  vec3 ink = vec3(0.014, 0.016, 0.02);
  vec3 slate = vec3(0.11, 0.12, 0.15);
  vec3 steel = vec3(0.08, 0.1, 0.13);
  vec3 col = mix(ink, slate, smoothstep(0.28, 0.72, n));
  col = mix(col, steel, ridges * 0.6);
  col *= 0.85 + 0.2 * n2;

  col *= 0.9 + 0.1 * sin(u_time * 0.13 + uv.x * 1.6);
  col += (hash(gl_FragCoord.xy + vec2(u_time * 8.0, u_time * 3.0)) - 0.5) * 0.015;
  o_color = vec4(col, 1.0);
}
