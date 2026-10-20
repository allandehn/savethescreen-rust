// name: Shoal
// blurb: Pool-floor light, constantly rewriting the highlights.

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

float caustic(vec2 p, float t) {
  vec2 q = p;
  p += 0.38 * vec2(sin(q.y * 1.9 + t), cos(q.x * 1.45 + t * 1.07));
  p += 0.22 * vec2(sin(q.y * 3.4 - t * 0.83), cos(q.x * 2.9 + t * 0.61));
  p += 0.1 * vec2(sin(q.x * 6.1 + t * 0.45), sin(q.y * 5.4 - t * 0.52));
  float c = abs(sin(p.x) * sin(p.y));
  return pow(c, 4.2);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  vec2 p = vec2(uv.x * aspect, uv.y);
  float t = iTime * 0.48;

  p += 0.05 * vec2(sin(t * 0.21), cos(t * 0.17));

  float depth = smoothstep(-0.05, 1.05, uv.y);
  float drift = 0.5 + 0.5 * sin(p.x * 2.2 + t * 0.3) * sin(p.y * 1.8 - t * 0.24);
  vec3 deep = mix(vec3(0.015, 0.06, 0.09), vec3(0.02, 0.08, 0.1), drift);
  vec3 shallow = vec3(0.04, 0.2, 0.22);
  vec3 col = mix(deep, shallow, 1.0 - depth);

  float c1 = caustic(p * 7.6, t);
  float c2 = caustic(p * 12.4 + 2.7, t * 0.82 + 1.7);
  float light = c1 * 0.85 + c2 * 0.65;
  light *= 0.5 + 0.5 * (1.0 - depth);

  vec3 sun = vec3(0.78, 0.95, 0.86);
  vec3 rim = vec3(0.16, 0.48, 0.52);
  col += rim * light * 0.8;
  col += sun * pow(light, 1.8) * 1.05;

  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 12.0, iTime * 6.0)) - 0.5) * 0.02;
  fragColor = vec4(col, 1.0);
}
