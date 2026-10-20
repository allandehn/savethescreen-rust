# Save the Screen

A native gallery of fullscreen shader savers. Each one keeps every part of the picture moving, so a panel never has to hold a still image of the last window you left up.

Open it and you get a shelf of the savers, live and small. Pick one and it fills the window. From there it can cover the monitor.

Ten savers ship with the app. To add another, save a [Shadertoy](https://www.shadertoy.com) `.frag` file in the shader folder and start the app again. The new card shows up without a rebuild.

The picture is drawn smaller than the screen, at 30 frames a second, then stretched to fit. A slow drift moves the whole frame across the panel, so even a quiet corner does not sit on the same pixels. When the window is covered or minimized, the clock pauses.

The browser version of this gallery lives at [savethescreen-html](https://github.com/allanjorch/savethescreen-html).

## Try it

```
savethescreen
```

On this machine, Super+Shift+L opens it. New windows land on workspace 1 and leave you on the workspace you are using.

| Key | What it does |
| --- | --- |
| 1–9, 0 | Open one of the first ten savers. In the gallery this is windowed. While a saver is up, you stay in fullscreen if you were already there. |
| Left, Right | Move to the previous or next saver. |
| Click a card | Open that saver, windowed. |
| Click, or F | Fullscreen on the current monitor. F toggles. A click enters fullscreen. |
| Esc | Leave fullscreen and stay on the saver. |
| ½ | Back to the gallery. On a keyboard without ½, the key just left of 1 does the same. |
| Wheel | Scroll the gallery when the window is too short for both rows. |

The built-in names, in that order:

1. Veil
2. Drift
3. Ember
4. Bodies
5. Shoal
6. Gloam
7. Cinder
8. Murk
9. Wisp
10. Silt

Gloam through Silt are the night set: the same families, kept dim.

## Add a shader

A saver is one Shadertoy image shader: a `.frag` file with `mainImage`. Paste the Image tab into a file, save it in the shader folder, and start the app again.

```
~/.config/savethescreen/shaders/
```

The folder is created the first time the app starts, and the path is printed in the terminal. `SAVETHESCREEN_SHADERS` points that folder somewhere else when you want a different place.

Three comment lines at the top set the card. Leave them out and the title comes from the file name, on the day row:

```glsl
// name: Harbor
// night: true
// blurb: A dim tide moving under the dock lights.

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float tide = 0.5 + 0.5 * sin(uv.x * 8.0 + iTime);
    fragColor = vec4(uv.x * 0.15, uv.y * 0.2, tide, 1.0);
}
```

`night: true` puts the card on the Night row. A file named `veil.frag` replaces the built-in Veil. Any other name is added after the built-ins. Send that same file to anyone running this app and they drop it in the same way.

The app fills in `iResolution`, `iTime`, `iTimeDelta`, `iFrameRate`, `iFrame`, `iMouse`, `iDate`, and `iSampleRate`. `iTime` pauses while the window is covered. `fragCoord` starts at the bottom left, as it does on Shadertoy.

Paste the single Image shader. A file that samples `iChannel0`, reads a second buffer, or uses the keyboard or sound is left off the shelf, and the reason is printed in the terminal. The other savers still run.

## Build

A current stable Rust toolchain. The release binary is what you want on your path.

```
cargo build --release
install -m 755 target/release/savethescreen ~/.local/bin/savethescreen
```
