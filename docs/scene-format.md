# Scene format navigation

The [full English format reference](../skills/layout-sketch/references/format.md) is the single detailed reference.

- [Scene and coordinates](../skills/layout-sketch/references/format.md#1-scene-and-coordinates)
- [Style and identity](../skills/layout-sketch/references/format.md#2-style-and-identity)
- [Primitives](../skills/layout-sketch/references/format.md#3-primitives)
- [Transforms](../skills/layout-sketch/references/format.md#4-transforms)
- [CLI and diagnostics](../skills/layout-sketch/references/format.md#5-cli-and-diagnostics)

The [JSON Schema](../skills/layout-sketch/references/scene.schema.json) checks structure; the binary also validates semantics and geometry. Stroke-inclusive bounds produce clipping warnings; hard geometry limits are errors even with `allow_clip`. See the [overview](../README.md#scene-overview).
