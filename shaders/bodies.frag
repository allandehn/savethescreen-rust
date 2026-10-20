// name: Bodies
// night: false
// blurb: Soft orbs on wandering paths, covering the whole rectangle.

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

vec2 path(float i, float t) {
  float px = fract(sin(i * 12.9898) * 43758.5453);
  float py = fract(sin(i * 78.233) * 23421.631);
  float wx = 0.09 + fract(sin(i * 4.1) * 61.0) * 0.18;
  float wy = 0.08 + fract(sin(i * 9.4) * 33.0) * 0.2;
  float ax = 0.32 + 0.18 * fract(sin(i * 2.3) * 17.0);
  float ay = 0.28 + 0.18 * fract(sin(i * 6.6) * 29.0);
  return vec2(
    0.5 + ax * sin(t * wx + px * 6.28318),
    0.5 + ay * sin(t * wy + py * 6.28318)
  );
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = vec2(fragCoord.x / iResolution.x, 1.0 - fragCoord.y / iResolution.y);
  float aspect = iResolution.x / iResolution.y;
  float t = iTime * 0.2;

  vec2 field = uv * vec2(aspect, 1.0);
  float wash = 0.5 + 0.5 * sin(field.x * 1.6 + t * 0.4) * sin(field.y * 1.8 - t * 0.33);
  wash += 0.2 * sin(field.x * 3.2 - t * 0.5 + field.y * 2.0);
  vec3 col = mix(vec3(0.03, 0.035, 0.08), vec3(0.09, 0.04, 0.1), wash);

  for (int i = 0; i < 12; i++) {
    float fi = float(i);
    vec2 c = path(fi + 1.0, t);
    vec2 dlt = (uv - c) * vec2(aspect, 1.0);
    float r = mix(0.04, 0.18, fract(sin(fi * 13.7) * 5.1));
    if (i < 4) r *= 1.55;
    float g = exp(-dot(dlt, dlt) / (r * r));
    vec3 a = vec3(0.92, 0.82, 0.58);
    vec3 b = vec3(0.42, 0.72, 0.92);
    vec3 d = vec3(0.78, 0.48, 0.95);
    vec3 pal = mix(a, b, fract(fi * 0.19));
    pal = mix(pal, d, fract(fi * 0.37));
    float mag = i < 4 ? 0.28 : 0.7;
    col += pal * g * mag;
  }

  col += (hash(vec2(fragCoord.x, iResolution.y - fragCoord.y) + vec2(iTime * 10.0, iTime * 4.0)) - 0.5) * 0.02;
  fragColor = vec4(col, 1.0);
}
