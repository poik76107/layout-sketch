# Gallery provenance

Each `NN-name.json` in `skills/layout-sketch/assets/examples/` maps to `NN-name.png` and `NN-name-preview.png` here. These are unedited renderer outputs: default preview scale 0.25, no grid. From the repository root, build and render into a fresh external directory, then copy only the finished documentation assets:

```sh
cargo build --release
SKETCH_DOC_IMAGES="$(mktemp -d)"
for scene in skills/layout-sketch/assets/examples/*.json; do
  stem="$(basename "$scene" .json)"
  ./target/release/sketch "$scene" -o "$SKETCH_DOC_IMAGES/$stem.png" \
    --preview "$SKETCH_DOC_IMAGES/$stem-preview.png" || exit 1
done
mkdir -p docs/images
cp "$SKETCH_DOC_IMAGES"/*.png docs/images/
```
