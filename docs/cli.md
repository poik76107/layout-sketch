# CLI guide

See the [complete captured help](#cli-help). Paths are relative to the current directory. Quote paths with spaces; prefix dash-leading paths with `./`, for example `-o ./-draft.png`. Create output parent directories before invoking the command. `-o` and `--output` are aliases; PNG is written regardless of extension.

`--preview` enables a preview, scaled by `--preview-scale` from 0.1 to 1 (default 0.25); dimensions round as floor(original × scale + 0.5). `--grid` adds a 10-unit grid only to the preview. Both `--grid` and `--preview-scale` require `--preview`. The main image stays clean. Inspect enlarged preview or main image when fine details are unclear.

Input, main and preview paths must refer to distinct files, including through aliases or links. Existing outputs are refused unless `--overwrite` is passed; the input is never overwritten. On Unix new files follow the process `umask`; replacement creates a new file under the current `umask`, rather than preserving the old mode.

Each file is published atomically, preview first and then main. A failure can leave a preview behind; the pair is not a transaction. Warnings alone are not failure. Use outputs only after exit 0 and expected OK lines: stale files after nonzero exit do not prove success. Exit 0 means success, 2 means invalid arguments or scene, and 3 means I/O or rendering failure. Warnings and OK go to stdout; errors go to stderr.

## CLI help

```text
sketch: render a JSON scene to PNG (schema 1)

Usage: sketch scene.json -o main.png [--preview preview.png] [--preview-scale S] [--grid] [--overwrite]
       sketch --help | --version

-o PATH, --output PATH   Main PNG (required)
--preview PATH           Optional preview PNG
--preview-scale S        Preview scale 0.1 to 1 (default 0.25); requires --preview
--grid                   Grid on preview only; requires --preview
--overwrite              Replace existing outputs

Primitive  Geometry
line       Segment by endpoints or angle and length
polyline   Open chain of points
polygon    Closed contour of points
rect       Rectangle
circle     Circle by center and radius
ellipse    Rotated ellipse
arc        Elliptical arc, optionally closed by a chord
curve      Cubic Bezier
path       M/L/Q/C/Z path data

Groups and repeat_rotate combine primitives. Scene coordinates use 100 units
on the short canvas side, with origin at top left and Y downward. Thick and
thin strokes are 9 and 4.5 pixels, with round caps and joins.
Painted paths after transforms and stroke expansion have outward-rounded bounds
of at most 16777216 pixels per axis, inclusive. This checks stroke outlines and
source paths with white or black fill; allow_clip does not bypass the limit.
```

## Reproducible diagnostics

From a fresh external working directory, set `SKETCH` to the absolute executable and `SCENE` to the absolute checkout path of `skills/layout-sketch/assets/examples/01-basic.json`, then run:

```sh
"$SKETCH" "$SCENE" -o main.png --preview preview.png
```

Actual stdout, exit 0:

```text
OK output=main.png size=1536x1024 warnings=0
OK preview=preview.png size=384x256 scale=0.25 grid=false
```

Create `warning.json` with:

```json
{"schema_version":1,"canvas":"square","shapes":[{"type":"line","id":"axis","from":[0,20],"to":[0,80]}]}
```

`"$SKETCH" warning.json -o warning.png` prints to stdout and exits 0:

```text
WARN OUTSIDE path=shapes[0] id=axis edge=left excess_px=4.5
OK output=warning.png size=1024x1024 warnings=1
```

Adding `"allow_clip":["left"]` to the line suppresses that warning. `"$SKETCH" warning.json --unknown` exits 2 and prints to stderr:

```text
ERROR argument: unknown option --unknown
```

Repeating the first command without `--overwrite` exits 3 and prints to stderr, without an OK line:

```text
ERROR output path=main.png: destination already exists
```
