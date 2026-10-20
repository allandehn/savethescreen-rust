// name: Veil
// blurb: Aurora curtains that keep sliding over a drifting sky.

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
    p = p * 2.04 + vec2(1.7, 9.2);
    a *= 0.5;
  }
  return v;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  vec2 p = vec2(uv.x * aspect, uv.y);
  float t = iTime * 0.07;

  p += 0.06 * vec2(sin(t * 0.37), cos(t * 0.29));

  float sky = 0.5 + 0.5 * sin(p.x * 0.8 + t * 0.4) * sin(p.y * 1.1 - t * 0.25);
  vec3 col = mix(vec3(0.02, 0.03, 0.07), vec3(0.05, 0.04, 0.1), sky);

  vec2 sp = uv * vec2(92.0, 52.0) + vec2(t * 1.6, t * 0.7);
  vec2 sf = fract(sp) - 0.5;
  float sh = hash(floor(sp));
  float star = smoothstep(0.035, 0.0, length(sf)) * smoothstep(0.86, 0.98, sh);
  star *= 0.45 + 0.55 * sin(iTime * (1.5 + sh * 3.4) + sh * 20.0);
  col += vec3(0.85, 0.9, 1.0) * star;

  for (int k = 0; k < 3; k++) {
    float fk = float(k);
    vec2 q = vec2(p.x * (0.85 + fk * 0.12), p.y * (1.35 + fk * 0.28));
    q.y -= t * (0.55 + fk * 0.12) + fk * 0.9;
    q.x += 0.45 * noise(vec2(q.x * 1.2, q.y * 0.7 + t));
    float n = fbm(q + 0.7 * noise(q * 1.6 + t * 0.4));
    float curtain = smoothstep(0.28, 0.78, n);
    curtain *= curtain;
    curtain *= smoothstep(-0.02, 0.08, uv.y) * mix(0.35, 1.0, smoothstep(1.12, 0.45, uv.y));
    curtain *= 0.55 + 0.45 * sin(p.x * 2.4 + n * 5.0 + fk);

    vec3 green = vec3(0.18, 0.92, 0.52);
    vec3 teal = vec3(0.12, 0.62, 0.85);
    vec3 violet = vec3(0.55, 0.22, 0.95);
    vec3 pal = mix(green, teal, fk / 2.0);
    pal = mix(pal, violet, smoothstep(0.45, 0.9, n));
    pal = mix(pal, vec3(0.75, 0.95, 0.7), pow(n, 4.0));
    col += pal * curtain * (0.85 - fk * 0.12);
  }

  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 11.0, iTime * 7.0)) - 0.5) * 0.02;
  fragColor = vec4(col, 1.0);
}
