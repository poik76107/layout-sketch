---
name: layout-sketch
description: Create a black-on-white layout sketch with the local sketch CLI before imagegen when object placement, silhouettes, poses, counts, or mechanism connections matter. Write coordinate-based scene JSON, render PNG plus a small preview, inspect and revise, then hand the clean full-size PNG to the available image-generation workflow. Also use for an explicitly requested layout reference. Requires the sketch binary and image-viewing capability; does not generate the final artwork.
---

# Layout Sketch

Produce a coordinate-authored layout reference. Use the installed sketch CLI for
rasterization. Keep materials, lighting and final artistic style in the imagegen prompt.
For a simple subject whose arrangement needs no control, skip this preparatory step
unless the user explicitly requests a sketch.

## 1. Establish available tools

- Locate sketch on PATH or use the executable path explicitly configured by the user.
  Run sketch --version; this skill targets sketch 0.1.x, schema 1.
- Use the host's file-writing, shell-execution and image-viewing tools. Names vary:
  Read, view_image or another actual image input tool may be available.
- Resolve this skill's references and assets relative to the directory containing
  this SKILL.md, not relative to the current project directory.
- If the binary is absent or incompatible, report the missing dependency. You may
  prepare scene JSON, but do not claim it was rendered or verified. Do not silently
  replace sketch with a generated Python drawing script.
- If the model or host cannot view images, report that visual review is unavailable.
  A successful render is not a visual check.

## 2. Plan and write the scene

Read [references/format.md](references/format.md) before authoring a scene.
Use [references/scene.schema.json](references/scene.schema.json) only when exact
field validation is needed; do not load the schema routinely.

Choose the canvas and make a brief plan of object regions, sizes and connections.
Use as few shapes as needed to communicate the requested geometry.
Use one compact JSON object per simple shape and stable diagnostic id values.

| Canvas | Scene coordinates | PNG | Default preview |
| --- | --- | --- | --- |
| landscape | 150×100 | 1536×1024 | 384×256 |
| portrait | 100×150 | 1024×1536 | 256×384 |
| square | 100×100 | 1024×1024 | 256×256 |

Use top-left origin, X right, Y down, and 10.24 pixels per scene unit.
Use circles for visible dots. Use curve control points to specify bends.
Use line angle/length or repeat_rotate when it avoids calculating trigonometry.
Use a group to move, rotate, uniformly scale or mirror a compound part.
Group transforms preserve the 9 px / 4.5 px marker widths.
Place foreground closed shapes later and fill them white to hide earlier lines.
Keep meaningful gaps; allow_clip is only for intentionally cropped scene elements.
Do not draw text, labels, decorations or style-reference details.

Copy only relevant patterns from [assets/examples/01-basic.json](assets/examples/01-basic.json),
[02-pulley.json](assets/examples/02-pulley.json),
[03-curves.json](assets/examples/03-curves.json),
[04-group-repeat.json](assets/examples/04-group-repeat.json),
[05-portrait.json](assets/examples/05-portrait.json) or
[06-square.json](assets/examples/06-square.json).
Treat examples as syntax and geometry patterns; do not insert their objects into
unrelated user requests.

Write scene_v01.json in the task's output directory, never in the installed skill.
Use schema_version: 1. The scene is ordinary JSON, not executable code.

## 3. Render and inspect

Create the output directory with the host's tools if needed, then run:

~~~bash
sketch scene_v01.json -o sketch_v01.png --preview sketch_preview_v01.png
~~~

Resolve paths against the command's working directory; quote paths containing spaces.
On nonzero exit, read the error, correct the scene or path, and rerun. Do not use
leftover files as evidence of success. On success, read OUTSIDE warnings and fix
unintended clipping.

Open sketch_preview_v01.png with the actual image-viewing tool. Check:

- Required objects, counts, sizes, placement and orientation.
- The intended endpoints, bends, connections and mechanism structure.
- Foreground/background order, silhouettes, gaps and unintended cropping.
- Readability without relying on labels or on the generator to repair geometry.

If a fine connection or thin line is ambiguous, rerender to fresh output names
with --preview-scale 0.5, or open the clean full-size PNG. A small preview is
not proof of fine geometric correctness.
For coordinate troubleshooting only, add --grid to a separately named preview.
Review a clean preview before handoff. Never pass a gridded preview as a reference.

Edit only the needed coordinates or group transform; save the next scene/output
version and repeat. Reinspect after every geometry change. Use --overwrite only
for deliberately replacing this task's disposable outputs.
Do not promise a fixed visual-token saving or an arbitrary maximum iteration count.

## 4. Hand off to imagegen

Read [references/handoff.md](references/handoff.md) for the handoff contract.
Supply the latest successfully rendered and visually reviewed **full-size PNG**
as the layout reference. Keep JSON and preview alongside it for future edits.

If imagegen called this skill, return the handoff and resume that caller; do not
invoke imagegen recursively. If the user requested a complete generated image,
continue through the available imagegen skill/tool after the sketch is ready.
If the user requested only a sketch, stop after delivering the sketch.

Load the actual imagegen instructions and follow their reference-image interface.
Do not invent a CLI, a reference argument, a model name, or a skill-calling API.
Never claim that imagegen was run when only the sketch was produced.
