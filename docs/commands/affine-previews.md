# Scale, Rotate, and Shear previews

[Transforms](transforms.md) · [Display caching](../viewport-caching.md)

Scale, Scale1D, Scale2D, Rotate, Rotate3D, and Shear preview their selected objects
while picking the final target. Numeric Scale1D factors preview while choosing
the direction, including negative and zero factors. Repeated Copy placements
preview the original sources; previously accepted copies remain model objects.

Target wires and points use the selection color. Shaded and ghosted target faces
retain the source object or layer color. Sources remain visible as wires:
Copy=Yes uses their display color and Copy=No uses gray. Copy changes update the
preview immediately. Object snaps, coordinate filters, and point constraints
feed the same resolved point into preview and acceptance. A filter awaiting its
coordinate source holds the previous preview. Leaving the viewports also holds
it. Completion or cancellation removes the preview.

An invalid final reference displays the original shape while the prompt stays
active. This includes a picked scale target at its base; numeric zero Scale1D
instead previews its collapsed directional map. Committing still checks model
geometry validity independently. If transforming a source bounding-box corner
would overflow, the whole source batch retains its previous preview.

[ScaleNU](scale-nu.md) previews staged axis factors and its current reference distance.
Its native mouse-preview fidelity is unproven.
[ScalePositions](scale-positions.md) shares the rigid display instances but caches
individual object centers regardless of grouping. Its previews preserve shape
and include translated bounds; native preview appearance remains unverified.

## Input geometry

- Scale1D free mouse picks follow the closest point on its reference line to
  the viewing ray. Typed and snapped targets use their distance projected onto
  that line. Targets on either side give a positive factor; negative scaling
  uses a numeric factor. A point target with zero projected distance is rejected.
- Rotate3D free mouse picks intersect the viewing ray with the plane normal to
  its axis through the axis start. Explicit snapped or filtered coordinates
  remain three-dimensional angle references.
- Rotate and Shear retain the construction plane of their first accepted pick.
  Scale2D uses the destination viewport's plane. Scale2D's reference-distance
  ratio includes its reference's normal displacement.

See [plane transforms](../plane-transforms.md) for the remaining off-plane rules.

## Rendering and verification

Motion transforms cached wires, meshes, and cofactor normals during scene
preparation. It does not clone model geometry, retessellate, edit selection,
change remembered defaults, or create history entries. Prepared maps handle
nonuniform scale, shear, reflections, and singular zero-factor display geometry.
Source smoothing groups are retained during preview; accepting the edit rebuilds
its display geometry as usual. Clipping includes the target bounds.

[60 prescribed cursor cases](../../tools/rhino_oracle/fixtures/affine_preview.json)
and [raw results and PNGs](../../tools/rhino_oracle/observations/affine_preview.json)
cover all six commands, Copy choices, all three display modes, perspective picks,
repeated copies, invalid final points, and negative/zero numeric Scale1D factors.
The zero-factor cases are canceled while pending, so they make no claim about
committing collapsed B-reps. [Six typed Scale1D cases](../../tools/rhino_oracle/fixtures/scale1d_projection.json)
retain [axis projection and zero-target results](../../tools/rhino_oracle/observations/scale1d_projection.json).

Rust replays the captured cameras and integer cursor inputs, comparing final
point/curve coordinates and B-rep vertices at absolute epsilon 1e-9. B-rep bounds
use 5e-7 because Rhino's shaded bounding cache includes render-mesh rounding.
Independent Python equations also check the raw geometry, framebuffer hashes,
image sizes, target point color, and source wire styles. These captures establish
behavior for the recorded witnesses, rather than pixel-identical rendering or
exhaustive command parity. GPU readback additionally exercises reflection and
nonuniform previews in linear and sRGB textures across all three display modes.

```sh
cargo test -p viboceros --bin viboceros affine_preview
cargo test -p viboceros --bin viboceros scale1d_typed_projection
cargo test -p viboceros-geometry transform::normal --lib
cargo test -p viboceros --bin viboceros gpu_affine_preview -- --ignored --nocapture
python3 -m unittest tools.rhino_oracle.test_affine_preview
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.affine_preview_input tools/rhino_oracle/fixtures/affine_preview.json --output /tmp/affine_preview.json --scheme VibocerosOracleMirrorPreview --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/scale1d_projection.json --scheme VibocerosOracleMirrorPreview --timeout 180
```

Every live capture uses the private Xvfb wrapper and an owned settings scheme.
Preview capture requires Pillow and xdotool on the host.
