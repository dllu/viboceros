# Transforming selected control points

[Control-point selection](control-points.md) · [Transforms](commands/transforms.md)

Run `PointsOn`, select control points, and use Move, Copy, Scale, Scale1D,
Scale2D, Rotate, Rotate3D, Mirror or Shear. The command acts on the selected
controls. Other controls on the same object retain their positions. If the
parent is also selected, its selected grips determine the edit. Ordinary
selected objects can be transformed alongside grip owners.

Command-first source selection accepts grips before Enter advances to the
point prompt. Its `SelAll` selects displayed grips and ordinary objects without
selecting the displayed parents. Shift adds picks and Ctrl removes them.
Automatic Scale centers include the selected grip locations.

Copy duplicates the entire owner and edits only the selected controls in that
copy. Original geometry stays intact. Both owners retain displayed, selected
grips; copied parent objects are unselected. Repeated interactive copies use
the original frozen source identities and grip indices, and share one Undo
entry. Move/Copy and affine point prompts show partial geometry previews in
all four viewports. Preview generation uses the same control-edit operation as
committed edits and leaves document geometry/history untouched.

## Geometry and history

Curve and surface edits preserve rational weights, degrees, knots and parameter
domains. Repeated closed and periodic seam controls alias their corresponding
unique grip. Surface grip order is U-major, while the control net's internal
storage is V-major. Closing an open curve can remove its final duplicated grip;
that vanished index is deselected. Undo restores the original endpoint grip and
its selection. Coincident mesh vertices remain independent controls.
Line edits produce a PolylineCurve with the original line domain; circle and
arc edits produce rational NURBS curves with the original domain.

The document validates the entire object/grip batch and stages every output
before mutation. Failed geometry edits preserve sources, selection, Undo and
Redo. Ordinary whole-object replacements precede grip-owner replacements in
the renewed object table. Geometry metadata and applicable copy group policy
use the existing document replacement/copy operations.

Grip replacement history exchanges its display and selected-index state with
the current state on replay. For example, after Move followed by PointsOff,
Undo restores the original geometry with the former selected grips displayed;
Redo restores the moved geometry with the display off. For Copy followed by
PointsOff, Undo removes the copy while the original display stays off; Redo
restores the copy with its display off. A zero Move produces no model Undo
entry. Preselected grip Copy retains its selected ordinary object peers through
Undo/Redo.

Viewport caches share immutable preview geometry across the four views. Cache
keys include source identity, selected indices, transform, tolerance and wire
density; camera changes reuse the geometry. Hidden sources are pruned from the
cache. This avoids repeating control edits and tessellation per viewport;
native performance equivalence has not been measured.

## Evidence and limits

Thirty-two Rhino 8.32.26160.13001 workflows were captured in an empty owned document
on private Xvfb. They cover clamped/rational/closed/periodic curves, lines,
closed polylines, circles, arcs, rational and closed surfaces, mesh vertices,
Move/Copy/Rotate/Scale/Mirror, parent precedence, mixed point objects, command-first
selection, endpoint closing, zero edits and PointsOff. EndCommand and final script states are
retained separately; completed macros have no idle cancellation tokens.
Application replay compares complete definitions, selection, display, object
order and Undo/Redo for 32 workflows through direct and incremental input at
absolute coordinate/weight/knot tolerance `1e-9`. Independent implementation tests
check command-first mouse picks, automatic Scale centers, repeated copies and
partial preview rendering in all three display modes.

`grip-transform-13` now retains the collapsed triangle `[4,2,3]` after Mirror,
matching the native face records, grip picks and Undo/Redo. Additional bounded
[mesh collapse/recovery diagnostics](mesh-edit-records.md) check topology,
colors, normals, area and closest points. Coincident peers remain independent.

Arbitrary nonrectangular trimmed single-face B-reps and composite PolyCurve
grip editing are unsupported. ScaleNU, other affine commands, Delete grips,
grip-specific snapping, grouped grip copy policy, native overlap menus, idle
Escape display cleanup, native mouse-preview fidelity and large control-net performance still need
implementation or verification. General closed/periodic, trimmed and composite
behavior is not established by these bounded recipes. Full Rhino parity remains
in progress.

## Reproduce

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino \
  tools/rhino_oracle/fixtures/grip_transform.json \
  --scheme VibocerosOracleGripTransform_20261004 \
  --output tools/rhino_oracle/observations/grip_transform.json --timeout 240
cargo test -p viboceros grip_transform_replays_native
python3 -m unittest tools.rhino_oracle.test_grip_transform
```

The live helper accepts only fixed bounded recipes, one iteration, a private
settings scheme and an empty owned document. The client requires a private
headless display before it can launch Rhino.
[Capture provenance](grip-transform-provenance.json)
