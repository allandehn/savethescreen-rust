// name: Silt
// night: true
// blurb: Deep water at night. Faint caustics and motes, barely there.

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

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  vec2 p = vec2(uv.x * aspect, uv.y);
  float t = iTime * 0.32;

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

  col *= 0.9 + 0.1 * sin(iTime * 0.12 + uv.x * 1.7);
  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 11.0, iTime * 5.0)) - 0.5) * 0.015;
  fragColor = vec4(col, 1.0);
}
