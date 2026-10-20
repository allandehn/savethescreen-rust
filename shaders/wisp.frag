// name: Wisp
// night: true
// blurb: Thin ghost-lights on near-black, stretching as they wander.

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

vec2 path(float i, float t) {
  float px = fract(sin(i * 12.9898) * 43758.5453);
  float py = fract(sin(i * 78.233) * 23421.631);
  float wx = 0.07 + fract(sin(i * 4.1) * 61.0) * 0.12;
  float wy = 0.06 + fract(sin(i * 9.4) * 33.0) * 0.14;
  float ax = 0.34 + 0.14 * fract(sin(i * 2.3) * 17.0);
  float ay = 0.3 + 0.14 * fract(sin(i * 6.6) * 29.0);
  return vec2(
    0.5 + ax * sin(t * wx + px * 6.28318),
    0.5 + ay * sin(t * wy + py * 6.28318)
  );
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  float t = iTime * 0.14;

  float wash = 0.5 + 0.5 * sin(uv.x * 1.4 + t * 0.28) * sin(uv.y * 1.6 - t * 0.22);
  vec3 col = mix(vec3(0.008, 0.01, 0.018), vec3(0.02, 0.024, 0.04), wash * 0.5);

  for (int i = 0; i < 8; i++) {
    float fi = float(i);
    vec2 c = path(fi + 1.0, t);
    vec2 dlt = (uv - c) * vec2(aspect, 1.0);
    float ang = t * (0.15 + fract(sin(fi * 3.7) * 5.0) * 0.2) + fi;
    float cs = cos(ang);
    float sn = sin(ang);
    dlt = vec2(cs * dlt.x - sn * dlt.y, sn * dlt.x + cs * dlt.y);
    dlt.x *= 0.22;
    float r = mix(0.08, 0.16, fract(sin(fi * 13.7) * 5.1));
    float g = exp(-dot(dlt, dlt) / (r * r));
    vec3 a = vec3(0.22, 0.32, 0.4);
    vec3 b = vec3(0.16, 0.28, 0.26);
    vec3 d = vec3(0.28, 0.2, 0.38);
    vec3 pal = mix(a, b, fract(fi * 0.19));
    pal = mix(pal, d, fract(fi * 0.37));
    col += pal * g * 0.38;
  }

  col *= 0.9 + 0.1 * sin(iTime * 0.1 + uv.y * 2.2);
  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 10.0, iTime * 4.0)) - 0.5) * 0.015;
  fragColor = vec4(col, 1.0);
}
