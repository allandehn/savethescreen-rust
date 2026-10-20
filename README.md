# Save the Screen

A native gallery of fullscreen shader savers. Each one keeps every part of the picture moving, so a panel never has to hold a still image of the last window you left up.

Open it and you get a shelf of the savers. Each card is a square. A card starts on its opening frame. Rest the pointer on one and it plays. Move on and it stays on the frame where you left it, and the next rest continues from there. Pick one and it fills the window, the same shape as the screen. From there it can cover the monitor.

The guidelines sit on their own view. `?`, or the bar at the bottom of the shelf, opens it. `?`, Esc, or Q brings you back to the pictures.

The shelf follows the Omarchy theme that is current while the app is open: its colors, its text size, and its monospace face. The bar along the bottom uses the theme's bar colors and its smaller body text. Change the theme and the shelf follows, without starting the app again.

A set of savers ships with the app. To add another, save a [Shadertoy](https://www.shadertoy.com) `.frag` file in the shader folder and start the app again. The new card joins the same shelf, sorted by name, without a rebuild.

A saver opens at 30 frames a second, and Page Up and Page Down step that pace up to the screen it is on. The picture is then stretched to fit. It also opens at half the window's pixels. The number keys run from 10% up to the full window. Both choices are remembered for each saver. A slow drift moves the whole frame across the panel, so even a quiet corner does not sit on the same pixels. When the window is covered or minimized, the clock pauses.

The browser version of this gallery lives at [savethescreen-html](https://github.com/allanjorch/savethescreen-html).

## Try it

```
savethescreen
```

On this machine, Super+Shift+L opens it. New windows land on workspace 1 and leave you on the workspace you are using.

| Key | What it does |
| --- | --- |
| Arrows, or h j k l | Move through the shelf. Left and right walk the names. Up and down move a row. |
| Enter | Open the highlighted card, windowed. On the remove question, Enter removes the shader. |
| Delete | On the shelf, ask before removing the highlighted shader. Enter removes it. Esc keeps it. |
| Click a card | Open that saver, windowed. A shader that needs a small edit opens its note instead. |
| ? | On the shelf, open the guidelines. Press it again to come back. On this keyboard, `?` is Shift and the `+` key. |
| Click ? - info | The bar at the bottom of the shelf. Opens the guidelines. |
| h, l, Left, Right | While a saver or its note is open, switch to the previous or next one. |
| 1–9, 0 | Set detail to 10% through 100% of the window. 0 is the full picture. On the shelf this sets the highlighted card. |
| Up, Down, or k j | While a saver is running, raise or lower detail by 10%. On the shelf, up and down move a row. |
| Page Up, Page Down | Step the pace. It starts at 30 fps and stops at this screen. On the shelf this sets the highlighted card. The card under the pointer still plays at 30 fps. |
| Click, or F | While a saver is running, fullscreen on the current monitor. F toggles. A click enters fullscreen. |
| Esc, or Q | From a saver, its note, or the guidelines, back to the gallery. From the gallery, quit. |
| ½ | Back to the gallery. On a keyboard without ½, the key just left of 1 does the same. |
| Wheel | Scroll the gallery, or the guidelines, when the window is too short. |

A saver opens at 50% and 30 frames a second. Rest on a card and that percent and pace sit in the lower right of the picture. A theme that names both a highlight background and a highlight foreground colors that pill. A theme that names both a selection background and a selection foreground does the same. Those two numbers are what the saver will use next time, on the card and while it runs. Both are kept in a `quality` file next to the shader folder. On the shelf, a card plays at 30 frames a second while the pointer rests on it, even when that card remembers a faster pace. Move the pointer off the card, or out of the window, and the card keeps the frame it had reached. Rest on it again and it continues from that frame. The others stay as they are. Once every visible card has its picture, the shelf rests.

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

Delete on the shelf asks before it removes the highlighted card. A file you added is deleted from the folder. A saver that ships with the app is named in `removed`, in that same folder, and the card stays off the shelf until you delete that line.

The app fills in `iResolution`, `iTime`, `iTimeDelta`, `iFrameRate`, `iFrame`, `iMouse`, `iDate`, `iSampleRate`, `iChannelTime`, and `iChannelResolution`. `iTime` pauses while the window is covered. `fragCoord` starts at the bottom left, as it does on Shadertoy.

Paste the single Image shader. A file that samples a channel still joins the shelf and runs. The picture, buffer, keyboard, or sound that channel wanted is a 256×256 noise picture: green is red shifted by 37 across and 17 down, which is the layout Clouds and the same family sample. A cubemap request gets that same picture on a cube. The card carries a small mark in the top right when a channel is using that picture, or when the shader needs a small edit.

A shader this app cannot compile stays on the shelf, with the same kind of mark. Hover says what happened. Open the card for a still note: the line to change, when this app can point at one, and what to try. The file is left as you saved it. Esc or Q brings you back to the shelf. The same note is printed where the app was started.

## Build

A current stable Rust toolchain. The release binary is what you want on your path.

```
cargo build --release
install -m 755 target/release/savethescreen ~/.local/bin/savethescreen
```
