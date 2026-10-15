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

float caustic(vec2 p, float t) {
  vec2 q = p;
  p += 0.34 * vec2(sin(q.y * 1.8 + t), cos(q.x * 1.4 + t * 1.05));
  p += 0.2 * vec2(sin(q.y * 3.2 - t * 0.8), cos(q.x * 2.8 + t * 0.58));
  float c = abs(sin(p.x) * sin(p.y));
  return pow(c, 4.0);
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_res.xy;
  float aspect = u_res.x / u_res.y;
  vec2 p = vec2(uv.x * aspect, uv.y);
  float t = u_time * 0.32;

  p += 0.04 * vec2(sin(t * 0.19), cos(t * 0.15));

  float depth = smoothstep(-0.05, 1.05, uv.y);
  float drift = 0.5 + 0.5 * sin(p.x * 1.8 + t * 0.22) * sin(p.y * 1.5 - t * 0.18);
  vec3 deep = mix(vec3(0.006, 0.016, 0.02), vec3(0.01, 0.028, 0.032), drift);
  vec3 col = mix(deep, vec3(0.018, 0.05, 0.055), (1.0 - depth) * 0.55);

  float c1 = caustic(p * 6.4, t);
  float c2 = caustic(p * 10.5 + 2.4, t * 0.78 + 1.4);
  float light = c1 * 0.55 + c2 * 0.4;
  light *= 0.45 + 0.4 * (1.0 - depth);
  col += vec3(0.08, 0.18, 0.2) * light * 0.7;
  col += vec3(0.22, 0.32, 0.3) * pow(light, 2.0) * 0.45;

  vec2 mote = uv * vec2(48.0, 28.0) + vec2(t * 1.1, -t * 1.8);
  float mh = hash(floor(mote));
  float spec = smoothstep(0.04, 0.0, length(fract(mote) - 0.5)) * smoothstep(0.93, 0.995, mh);
  col += vec3(0.18, 0.28, 0.26) * spec * 0.5;

  col *= 0.9 + 0.1 * sin(u_time * 0.12 + uv.x * 1.7);
  col += (hash(gl_FragCoord.xy + vec2(u_time * 11.0, u_time * 5.0)) - 0.5) * 0.015;
  o_color = vec4(col, 1.0);
}
