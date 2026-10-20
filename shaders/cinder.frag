// name: Cinder
// night: true
// blurb: Coals in the dark. Heat that stays low and keeps drifting.

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

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  float t = iTime * 0.16;

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

  col *= 0.9 + 0.1 * sin(iTime * 0.11 + uv.y * 1.8);
  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 7.0, iTime * 3.0)) - 0.5) * 0.015;
  fragColor = vec4(col, 1.0);
}
