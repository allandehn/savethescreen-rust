# Later

## Import from Shadertoy

Bring a shader in from [shadertoy.com](https://www.shadertoy.com): the picture, the name, the description, and the author. A link or an id is enough. The app saves the `.frag` in the shader folder, with the card comments filled in.

Parked for now. The shelf already takes a `.frag` you paste in yourself.

## Keep the compile note

When a drop-in fails to compile, the card stays on the shelf. Hover says what happened. Opening the card shows a still note. For `mat2` built from one `vec4`, the note gives a `twist` helper and the line to write. Any other failure quotes the line Naga names, and says the shader may still run on Shadertoy. The file on disk is left as saved. This lives in `explain_compile` in `src/render.rs`. Keep it. It is the path for showing someone what to change.

## Adapt a shader and say so

A general pass that rewrites the copy this app compiles, runs that copy, and tells the person what changed. They would not edit the file. The card would still say that something was adapted.

Parked. Each gap is its own small rewrite, such as `textureLod` on a flat picture with a vec3 coordinate, or the `mat2` turn above. A fixer that covers Shadertoy in general is larger than this app. The note above is what we use until this is worth building.
