# sketch 0.1.x: scene and CLI reference

## Contents

1. Scene and coordinates
2. Style and identity
3. Primitives
4. Transforms
5. CLI and diagnostics

## 1. Scene and coordinates

~~~json
{"schema_version": 1, "canvas": "landscape", "shapes": [
  {"id": "wheel", "type": "circle", "cx": 40, "cy": 50, "r": 20},
  {"id": "rope", "type": "line", "from": [40, 30], "to": [100, 30], "width": "thin"}
]}
~~~

schema_version: 1 and a nonempty shapes array are required; canvas defaults to
landscape. Unknown fields and duplicate JSON keys are errors.
There is no root style object, no variables, no references, and no expressions.

| canvas | Logical width × height | Main PNG | Preview at 0.25 |
| --- | --- | --- | --- |
| landscape | 150×100 | 1536×1024 | 384×256 |
| portrait | 100×150 | 1024×1536 | 256×384 |
| square | 100×100 | 1024×1024 | 256×256 |

All geometric coordinates and lengths are logical units, 10.24 px per unit.
Fractional and negative local coordinates are accepted. X points right, Y down.
Angles are degrees clockwise: 0 right, 90 down.
Numbers are first decoded to finite f64 (binary64), with rounding. Domain checks,
including schema_version equality and count integrality/range, use the decoded
values, not exact decimal tokens. Integral spellings such as 6, 6.0 and 6e0 and
tokens rounded to permitted values are accepted; overflow to infinity is rejected.
Rendering also requires representable intermediate and transformed coordinates.
Positive dimensions/radii/lengths are required.

## 2. Style and identity

Primitive fields: width is thick (default, 9 px) or thin (4.5 px);
fill is none (default), white or black. Black stroke and white canvas are fixed.
Stroke widths stay constant under group scaling. Caps and joins are round.
Each shape is filled first, then stroked; array order is back to front.
Closed self-intersecting paths use even-odd fill.

id is optional, diagnostic only, globally unique in source JSON. Use 1–64 ASCII
characters: first a letter, remaining letters/digits/underscore/hyphen.
Id values do not appear in images. Do not use role as an alias.

allow_clip may list left, top, right, bottom on any node. It suppresses only
intentional clipping warnings on those sides. Containers pass their allowed
sides to all descendants; a child adds to, rather than removes from, that set.

Open line/polyline/curve and open arc accept fill: none only.
group and repeat_rotate have no fill or width; set styles on their children.
No color, opacity, text, dash or stroke:none fields are supported.

## 3. Primitives

### Line

~~~json
{"type": "line", "from": [10, 20], "to": [35, 40]}
{"type": "line", "from": [10, 20], "angle": 45, "length": 20}
~~~

Choose exactly one form. Never combine to with angle/length.
The endpoint form needs distinct endpoints. The polar form needs length > 0.

### Polyline / polygon

~~~json
{"type": "polyline", "pts": [[10, 10], [20, 25], [40, 15]], "width": "thin"}
{"type": "polygon", "pts": [[10, 10], [20, 25], [40, 15]], "fill": "white"}
~~~

polyline needs at least 2 points and stays open.
polygon closes automatically; appending the first point is unnecessary.
One terminal copy of the first point is accepted. Exclude only that one marker
for validation and preserve the source list. The remaining contour needs at least
3 points, no adjacent duplicates (including last-to-first), and exact noncollinearity
over decoded binary64 coordinates, without a tolerance. Choose the first point and
the first different point as the collinearity baseline. Zero signed area alone
does not make a self-intersecting polygon invalid. Do not repeatedly remove points.
Polyline has no closing-marker exception: adjacent duplicates are invalid, but
collinear polylines are accepted.

### Rectangle / circle / ellipse

~~~json
{"type": "rect", "x": 10, "y": 20, "w": 30, "h": 15, "fill": "white"}
{"type": "circle", "cx": 50, "cy": 50, "r": 15}
{"type": "ellipse", "cx": 50, "cy": 50, "rx": 20, "ry": 12, "angle": 30}
~~~

rect uses top-left x/y and positive w/h. Rotate a rectangle with group.
ellipse uses positive rx/ry; angle rotates around its center and defaults to 0.
To draw a visible point, use a small black-filled circle. There is no point command.

### Arc

~~~json
{"type": "arc", "cx": 50, "cy": 50, "rx": 20, "ry": 12,
 "start": 0, "sweep": 180, "angle": 0, "chord": true, "fill": "white"}
~~~

Required: center, both radii, start and sweep.
angle defaults to 0, chord defaults to false.
The local ellipse is (cx + rx*cos(t), cy + ry*sin(t)); angle rotates it about its center.
start and sweep refer to parameter t, not the polar ray angle of a noncircular ellipse.
Use 0 < abs(sweep) <= 360. Positive sweep is clockwise.
chord:false leaves the curve open and requires fill:none.
chord:true joins endpoints with a straight segment; it does not connect them to the center.

### Cubic curve

~~~json
{"type": "curve", "from": [10, 50], "ctrl1": [20, 30],
 "ctrl2": [40, 30], "to": [50, 50]}
~~~

All four points are required. Control points shape the bend and need not lie on
the visible curve. This primitive is open. Use path for a closed curved shape.

### Path

~~~json
{"type": "path", "d": "M 10 50 Q 30 20 50 50 L 50 65 L 10 65 Z", "fill": "white"}
~~~

Only absolute uppercase M, L, Q, C, Z are supported:
M x y; L x y; Q cx cy x y; C c1x c1y c2x c2y x y; Z.
Write every command explicitly with exactly one argument set.
Begin with M; begin a new subpath with M. After Z, use a new M or finish.
Every subpath needs at least one segment.
Spaces, newlines, SVG-style commas, signed decimal numbers and exponents are accepted.
Trailing commas, extra arguments, lowercase commands, H/V/S/T/A, implicit command
repetition, and SVG/XML documents are errors.
For white/black fill, close every subpath explicitly with Z.

## 4. Transforms

### Group

~~~json
{"type": "group", "id": "assembly", "at": [70, 50], "rotate": 30,
 "scale": 1.2, "flip_x": false, "shapes": [
  {"type": "circle", "cx": 0, "cy": 0, "r": 8, "fill": "white"},
  {"type": "line", "from": [0, 0], "to": [15, 0]}
]}
~~~

Defaults: at:[0,0], rotate:0, scale:1, flip_x:false.
Transform order: mirror local X → uniform positive scale → rotate → translate.
The transform origin is local [0,0]. Keep stroke widths unchanged.
group is allowed only at the root. Children may be primitives or repeat_rotate.
There are no nested groups, named templates, instances, anchors or pose overrides.

### Radial repeat

~~~json
{"type": "repeat_rotate", "id": "spokes", "center": [40, 50],
 "count": 6, "start": 30, "shapes": [
  {"type": "line", "from": [0, 0], "to": [18, 0], "width": "thin"}
]}
~~~

center and integer count (1..256) are required. start defaults to 0.
step defaults to 360/count; an explicit finite step overrides it.
Child coordinates are local around [0,0].
Copy i maps p to center + rotate(start + i*step, p), for i=0..count-1.
An enclosing group is applied afterwards.
Only primitive children are allowed. Paint all children of one copy before the next.

## 5. CLI and diagnostics

~~~bash
sketch scene_v01.json -o sketch_v01.png --preview sketch_preview_v01.png
sketch scene_v02.json -o sketch_v02.png --preview sketch_preview_v02.png --preview-scale 0.5
~~~

--output is an alias for -o. --preview is optional for the binary, but request it
for the skill's normal review workflow. --preview-scale defaults to 0.25, range 0.1..1,
and requires --preview. Dimensions round as floor(original*scale + 0.5).
--grid requires --preview and adds a light gray 10-unit grid only to that preview.
It adds no coordinate text and does not affect the main PNG.
Use --overwrite only to intentionally replace existing output files.
No --canvas flag exists; select canvas in JSON.

Output parents must already exist. Input/output/preview must be different files.
Without --overwrite, existing outputs cause an error. Never overwrite the input.
--version reports sketch 0.1.x (schema 1). --help shows usage.

Exit 0: success, possibly with WARN OUTSIDE.
Exit 2: invalid arguments/JSON/geometry or exceeded input limits.
Exit 3: I/O or rendering error, including an existing output without --overwrite.
Errors go to stderr. Warnings and final OK lines go to stdout.
Two output files are not a transaction: a failed invocation may leave one new file.
Use files only after exit 0 and the expected OK lines.

Bound checks include stroke width. No overlap, small-object or physics checks exist.
Input limit: 4 MiB; at most 10,000 expanded primitives; path d at most 65,536 bytes.
After transforms and stroke expansion, each painted path’s outward-rounded bounds
may span at most 16777216 px (1638400 scene units) on either axis, inclusive.
This checks every stroke outline and the source path for white or black fill;
excess is an addressed geometry error, even with allow_clip.
The full-size PNG has a white background and antialiased black geometry.
The preview is a Lanczos3 resize of that same rendered image; thin lines and gaps
may become unclear. Enlarge the preview or inspect the full-size image when needed.
