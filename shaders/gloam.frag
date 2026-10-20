// name: Gloam
// blurb: A dim twilight veil. Movement you notice after a breath.

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
    p = p * 2.03 + vec2(1.7, 9.2);
    a *= 0.5;
  }
  return v;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  vec2 p = vec2(uv.x * aspect, uv.y);
  float t = iTime * 0.045;

  p += 0.05 * vec2(sin(t * 0.33), cos(t * 0.27));

  float sky = 0.5 + 0.5 * sin(p.x * 0.7 + t * 0.35) * sin(p.y * 0.9 - t * 0.22);
  vec3 col = mix(vec3(0.008, 0.01, 0.02), vec3(0.02, 0.018, 0.04), sky * 0.55);

  vec2 sp = uv * vec2(70.0, 40.0) + vec2(t * 0.8, t * 0.35);
  float sh = hash(floor(sp));
  float star = smoothstep(0.03, 0.0, length(fract(sp) - 0.5)) * smoothstep(0.9, 0.99, sh);
  star *= 0.4 + 0.6 * sin(iTime * (1.1 + sh * 2.4) + sh * 12.0);
  col += vec3(0.35, 0.4, 0.55) * star * 0.45;

  for (int k = 0; k < 3; k++) {
    float fk = float(k);
    vec2 q = vec2(p.x * (0.8 + fk * 0.1), p.y * (1.2 + fk * 0.22));
    q.y -= t * (0.4 + fk * 0.1) + fk * 0.7;
    q.x += 0.4 * noise(vec2(q.x * 1.1, q.y * 0.6 + t));
    float n = fbm(q + 0.6 * noise(q * 1.5 + t * 0.35));
    float curtain = smoothstep(0.32, 0.8, n);
    curtain *= curtain;
    curtain *= smoothstep(-0.02, 0.12, uv.y) * mix(0.4, 1.0, smoothstep(1.1, 0.4, uv.y));
    vec3 pal = mix(vec3(0.12, 0.24, 0.32), vec3(0.2, 0.12, 0.32), fk / 2.0);
    pal = mix(pal, vec3(0.1, 0.26, 0.22), n);
    col += pal * curtain * (0.7 - fk * 0.1);
  }

  col *= 0.9 + 0.1 * sin(iTime * 0.12 + uv.x * 2.0);
  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 9.0, iTime * 4.0)) - 0.5) * 0.015;
  fragColor = vec4(col, 1.0);
}
