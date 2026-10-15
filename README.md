# Save the Screen

A native gallery of ten fullscreen shader savers. Each one keeps every part of the picture moving, so a panel never has to hold a still image of the last window you left up.

Open it and you get a shelf of the savers, live and small. Pick one and it fills the window. From there it can cover the monitor.

The picture is drawn smaller than the screen, at 30 frames a second, then stretched to fit. A slow drift moves the whole frame across the panel, so even a quiet corner does not sit on the same pixels. When the window is covered or minimized, the clock pauses.

The browser version of this gallery lives at [savethescreen-html](https://github.com/allanjorch/savethescreen-html).

## Try it

```
savethescreen
```

On this machine, Super+Shift+L opens it. New windows land on workspace 1 and leave you on the workspace you are using.

| Key | What it does |
| --- | --- |
| 1–9, 0 | Open that saver. In the gallery this is windowed. While a saver is up, you stay in fullscreen if you were already there. |
| Click a card | Open that saver, windowed. |
| Click, or F | Fullscreen on the current monitor. F toggles. A click enters fullscreen. |
| Esc | Leave fullscreen and stay on the saver. |
| ½ | Back to the gallery. On a keyboard without ½, the key just left of 1 does the same. |
| Wheel | Scroll the gallery when the window is too short for both rows. |

The names, in that order:

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

## Build

A current stable Rust toolchain. The release binary is what you want on your path.

```
cargo build --release
install -m 755 target/release/savethescreen ~/.local/bin/savethescreen
```
