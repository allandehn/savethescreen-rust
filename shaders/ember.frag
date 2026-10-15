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

vec2 blob(float i, float t) {
  float px = fract(sin(i * 12.9898) * 43758.5453);
  float py = fract(sin(i * 78.233) * 23421.631);
  float wx = 0.13 + fract(sin(i * 3.17) * 91.7) * 0.22;
  float wy = 0.11 + fract(sin(i * 5.91) * 47.2) * 0.24;
  float x = 0.5 + 0.48 * sin(t * wx + px * 6.28318);
  float y = 0.5 + 0.44 * sin(t * wy + py * 6.28318);
  return vec2(x, y);
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_res.xy;
  float aspect = u_res.x / u_res.y;
  float t = u_time * 0.24;

  float field = 0.5 + 0.5 * sin(uv.x * 3.4 + t * 0.55) * sin(uv.y * 2.7 - t * 0.42);
  field += 0.25 * sin((uv.x + uv.y) * 5.0 - t * 0.7);
  vec3 col = mix(vec3(0.05, 0.015, 0.01), vec3(0.12, 0.04, 0.015), field);

  for (int i = 0; i < 8; i++) {
    float fi = float(i);
    vec2 c = blob(fi + 1.0, t);
    vec2 dlt = (uv - c) * vec2(aspect, 1.0);
    float r = 0.22 + 0.15 * fract(sin(fi * 19.1) * 8.3);
    r *= 0.9 + 0.1 * sin(t * 0.8 + fi);
    float g = exp(-dot(dlt, dlt) / (r * r));
    vec3 coal = vec3(0.55, 0.08, 0.02);
    vec3 flame = vec3(0.98, 0.48, 0.12);
    vec3 gold = vec3(1.0, 0.82, 0.4);
    vec3 pal = mix(coal, flame, fract(fi * 0.173 + 0.2));
    pal = mix(pal, gold, pow(g, 2.4));
    col += pal * g * (0.5 + 0.1 * sin(t + fi));
  }

  col += (hash(gl_FragCoord.xy + vec2(u_time * 8.0, u_time * 3.0)) - 0.5) * 0.02;
  o_color = vec4(col, 1.0);
}
