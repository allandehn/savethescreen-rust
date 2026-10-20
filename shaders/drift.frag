// name: Drift
// blurb: Ink in water. Filaments crawl so no region sits still.

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
    p = p * 2.03 + vec2(3.1, 1.4);
    a *= 0.5;
  }
  return v;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  vec2 p = vec2(uv.x * aspect, uv.y) * 1.35;
  float t = iTime * 0.038;

  p += vec2(t * 0.17, -t * 0.11);

  vec2 q = p;
  q += 0.9 * vec2(fbm(q + t), fbm(q + vec2(5.2, 1.3) - t));
  q += 0.4 * vec2(noise(q * 1.6 - t * 0.65), noise(q * 1.5 + t * 0.55));

  float n = fbm(q * 1.35);
  float n2 = noise(q * 2.1 + 8.0);
  float ridges = pow(1.0 - abs(n * 2.0 - 1.0), 2.6);

  vec3 ink = vec3(0.055, 0.05, 0.045);
  vec3 sand = vec3(0.72, 0.56, 0.38);
  vec3 tide = vec3(0.2, 0.4, 0.42);
  vec3 rust = vec3(0.52, 0.2, 0.16);

  vec3 col = mix(ink, sand, smoothstep(0.28, 0.72, n));
  col = mix(col, tide, ridges * 0.82);
  col = mix(col, rust, smoothstep(0.55, 0.9, n2) * 0.42);
  col *= 0.78 + 0.22 * n2;

  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 9.0, iTime * 5.0)) - 0.5) * 0.02;
  fragColor = vec4(col, 1.0);
}
