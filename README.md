# Save the Screen

A native gallery of fullscreen shader savers. Each one keeps every part of the picture moving, so a panel never has to hold a still image of the last window you left up.

Open it and you get a shelf of the savers. Each picture is the same shape as the screen the window is on. A card starts on its opening frame. Rest the pointer on one and it plays. Move on and it stays on the frame where you left it, and the next rest continues from there. Pick one and it fills the window. From there it can cover the monitor.

A set of savers ships with the app. To add another, save a [Shadertoy](https://www.shadertoy.com) `.frag` file in the shader folder and start the app again. The new card joins the same shelf, sorted by name, without a rebuild.

The picture is drawn at 30 frames a second, then stretched to fit. It opens at half the window's pixels. The number keys run from 10% up to the full window, and that choice is remembered for each saver. A slow drift moves the whole frame across the panel, so even a quiet corner does not sit on the same pixels. When the window is covered or minimized, the clock pauses.

The browser version of this gallery lives at [savethescreen-html](https://github.com/allanjorch/savethescreen-html).

## Try it

```
savethescreen
```

On this machine, Super+Shift+L opens it. New windows land on workspace 1 and leave you on the workspace you are using.

| Key | What it does |
| --- | --- |
| Arrows, or h j k l | Move through the shelf. Left and right walk the names. Up and down move a row. |
| Enter | Open the highlighted card, windowed. |
| Click a card | Open that saver, windowed. A shader that needs a small edit opens its note instead. |
| h, l, Left, Right | While a saver or its note is open, switch to the previous or next one. |
| 1–9, 0 | Set detail to 10% through 100% of the window. 0 is the full picture. On the shelf this sets the highlighted card. |
| Up, Down, or k j | While a saver is running, raise or lower detail by 10%. On the shelf, up and down move a row. |
| Click, or F | While a saver is running, fullscreen on the current monitor. F toggles. A click enters fullscreen. |
| Esc, or Q | From a saver or its note, back to the gallery. From the gallery, quit. |
| ½ | Back to the gallery. On a keyboard without ½, the key just left of 1 does the same. |
| Wheel | Scroll the gallery when the window is too short for the shelf. |

A saver opens at 50%. The percent on the highlighted card, and on the saver, is the step it will use next time too. That choice is kept in a `quality` file next to the shader folder. On the shelf, a card plays at 30 frames a second while the pointer rests on it. Move the pointer off the card, or out of the window, and the card keeps the frame it had reached. Rest on it again and it continues from that frame. The others stay as they are. Once every visible card has its picture, the shelf rests.

The built-in names, in shelf order:

- Bodies
- Cinder
- Drift
- Ember
- Gloam
- Murk
- Shoal
- Silt
- Veil
- Wisp

## Add a shader

A saver is one Shadertoy image shader: a `.frag` file with `mainImage`. Paste the Image tab into a file, save it in the shader folder, and start the app again.

```
~/.config/savethescreen/shaders/
```

The folder is created the first time the app starts, and the path is printed in the terminal. `SAVETHESCREEN_SHADERS` points that folder somewhere else when you want a different place.

Two comment lines at the top set the card. Leave them out and the title comes from the file name:

```glsl
// name: Harbor
// blurb: A dim tide moving under the dock lights.

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float tide = 0.5 + 0.5 * sin(uv.x * 8.0 + iTime);
    fragColor = vec4(uv.x * 0.15, uv.y * 0.2, tide, 1.0);
}
```

A file named `veil.frag` replaces the built-in Veil. Any other name is added to the shelf, and the cards are sorted by title. Send that same file to anyone running this app and they drop it in the same way.

The app fills in `iResolution`, `iTime`, `iTimeDelta`, `iFrameRate`, `iFrame`, `iMouse`, `iDate`, `iSampleRate`, `iChannelTime`, and `iChannelResolution`. `iTime` pauses while the window is covered. `fragCoord` starts at the bottom left, as it does on Shadertoy.

Paste the single Image shader. A file that samples a channel still joins the shelf and runs. The picture, buffer, keyboard, or sound that channel wanted is a 256×256 noise picture: green is red shifted by 37 across and 17 down, which is the layout Clouds and the same family sample. A cubemap request gets that same picture on a cube. The card carries a small mark in the corner. Hover the card and it names the slot.

A shader this app cannot compile stays on the shelf, with the same kind of mark. Hover says what happened. Open the card for a still note: the line to change, when this app can point at one, and what to try. The file is left as you saved it. Esc or Q brings you back to the shelf. The same note is printed where the app was started.

## Build

A current stable Rust toolchain. The release binary is what you want on your path.

```
cargo build --release
install -m 755 target/release/savethescreen ~/.local/bin/savethescreen
```
