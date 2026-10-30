# Handoff to an image-generation workflow

Return these facts to the calling agent or skill:

- Absolute path to the reviewed clean full-size PNG.
- Absolute paths to its source scene JSON and preview.
- Canvas name and actual PNG dimensions.
- Brief mapping of shapes to intended objects and required connections/counts.
- Any unresolved visual ambiguity or intentional cropping.
- Review status: preview reviewed, enlarged/full-size reviewed if necessary, or not reviewed.

Use the latest successful matching scene/render pair. A filename alone is not proof
that a render succeeded. Never mark an unviewed image as reviewed.

## Reference role

Include this role in the image-generation prompt, adapting numbering to the actual tool:

> Image 1 is a layout sketch. It specifies object placement, relative sizes,
> silhouettes, poses and connections. Follow its layout and mechanism structure
> closely. It is not a style reference: do not reproduce the sketch's black
> outlines or white background unless the requested final style calls for them.

Explain what ambiguous lines represent. Specify important object counts and
connections. Describe final materials, lighting, palette and style separately.
If style/character references are available, give them separate roles after the layout.
Do not assume the generator has seen scene JSON, ids, the conversation or the preview.
Do not promise exact geometric reproduction by an image generator.

## Tool adaptation

Read the installed imagegen skill and use its actual reference interface:

- For an interface accepting local image paths, pass the clean full-size PNG as the
  first reference. Resolve paths from the calling environment.
- For an interface requiring attached image identifiers, use the host's supported
  attachment mechanism for the clean PNG; do not substitute the preview.
- Do not invent --ref, referenced_image_paths, model names or a CLI executable.
  Use such arguments only if the available imagegen interface actually defines them.
- If that workflow requires viewing the full-size reference before use, follow it.
  Otherwise the clean preview, enlarged when necessary, is the default review surface.

## Avoid recursive skill calls

If imagegen requested the sketch, return the handoff and let that imagegen workflow
resume. If layout-sketch was invoked first for a complete image request, load
imagegen once after review and provide the handoff as already completed.
Do not rebuild a valid reviewed sketch solely because imagegen's preflight mentions
layout-sketch. Rebuild only when the intended layout changes or the files are stale.

For edits to an existing final image, follow imagegen's editing instructions.
A new layout sketch is needed when composition/geometry changes require it, not on
every color or material edit. If only a sketch was requested, do not start generation.
