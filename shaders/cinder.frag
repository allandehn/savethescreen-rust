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
  float wx = 0.1 + fract(sin(i * 3.17) * 91.7) * 0.16;
  float wy = 0.08 + fract(sin(i * 5.91) * 47.2) * 0.18;
  return vec2(
    0.5 + 0.46 * sin(t * wx + px * 6.28318),
    0.5 + 0.42 * sin(t * wy + py * 6.28318)
  );
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_res.xy;
  float aspect = u_res.x / u_res.y;
  float t = u_time * 0.16;

  float field = 0.5 + 0.5 * sin(uv.x * 2.6 + t * 0.4) * sin(uv.y * 2.1 - t * 0.32);
  vec3 col = mix(vec3(0.012, 0.006, 0.004), vec3(0.04, 0.014, 0.008), field * 0.55);

  for (int i = 0; i < 7; i++) {
    float fi = float(i);
    vec2 c = blob(fi + 1.0, t);
    vec2 dlt = (uv - c) * vec2(aspect, 1.0);
    float r = 0.16 + 0.1 * fract(sin(fi * 19.1) * 8.3);
    r *= 0.9 + 0.1 * sin(t * 0.7 + fi);
    float g = exp(-dot(dlt, dlt) / (r * r));
    vec3 pal = mix(vec3(0.28, 0.05, 0.015), vec3(0.42, 0.16, 0.04), fract(fi * 0.19));
    pal = mix(pal, vec3(0.55, 0.28, 0.08), pow(g, 2.6));
    col += pal * g * (0.22 + 0.06 * sin(t + fi));
  }

  col *= 0.9 + 0.1 * sin(u_time * 0.11 + uv.y * 1.8);
  col += (hash(gl_FragCoord.xy + vec2(u_time * 7.0, u_time * 3.0)) - 0.5) * 0.015;
  o_color = vec4(col, 1.0);
}
