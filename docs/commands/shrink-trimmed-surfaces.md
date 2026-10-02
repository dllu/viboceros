# Shrink trimmed surfaces

[Command reference](README.md)

`ShrinkTrimmedSrf` reduces each selected surface's underlying UV domain to the
outer trim bounds, leaving a 1% margin around non-isoparametric extrema.
`ShrinkTrimmedSrfToEdge` omits that margin. Isoparametric sides shrink exactly to
their trim coordinates. Holes do not set the shrink bounds.

Both commands accept whole surfaces and polysurfaces. With preselection they
execute immediately and preserve selection. Otherwise select objects and press
Enter; changed objects are deselected, while already-shrunk objects remain
selected. Escape during picking leaves geometry untouched.

`ShrinkTrimmedSrf` also accepts face preselection. Ctrl/Command+Shift clicks or
rectangles pick faces before or during its selection prompt. Only those faces
shrink; neighboring faces and shared spatial edges remain unchanged. Face picks
are cleared after execution, including already-shrunk faces. Whole objects and
faces on other objects can share one edit. `ShrinkTrimmedSrfToEdge` ignores face
preselection and accepts whole objects only, matching the recorded native runs.

Scripts can address faces directly (zero-based indices):

```text
ShrinkTrimmedSrf <object-id> 0,2
ShrinkTrimmedSrf <first-object-id> 1 <second-object-id> 0
```

Knot insertion and surface cropping retain the visible trim image without
fitting. UV coordinates, orientation, vertices, spatial edges, trim curves
(including their weights), topology indices, object IDs, attributes, and groups
are preserved. Underlying surface knots and control nets are cropped. Cached
isoparametric classes are updated for the new domains.
All objects are staged before editing; one Undo restores the batch. An unchanged
batch preserves history and the Redo branch. Undo/Redo release command-first
picks, including unchanged selected peers.

## Geometry API

```rust
let result = brep.try_shrunk_surfaces(
    viboceros_geometry::BrepSurfaceShrinkMode::Standard,
    tolerance,
)?;
```

Use `BrepSurfaceShrinkMode::ToEdge` for the tighter crop. UV bounds use adaptive
rational hull refinement in translated parameter coordinates. Straight trim
bounds are exact; general curve bounds have numerical refinement error. Poles,
exhausted refinement budgets, and failed validation return errors. Surface
cropping uses the existing NURBS knot algebra. The relative stopping rule prevents
repeated tiny changes independently of the document's model-space tolerance.

`try_shrunk_surface_faces(&[0, 2], mode, tolerance)` limits the geometry operation
to specific faces; duplicates are idempotent and invalid indices fail before
cropping. The command crate's `ShrinkTrimmedSelection` stages explicit face
targets together with eligible whole-object selection. Its commit rejects
changed source geometry or tolerance and preserves Redo on a no-op.

## Verification and limits

154 saved Rhino 8.32 command cases cover both whole-object selection workflows,
rectangular and curved trims, holes, rotated circles and ellipses, cubic polynomial and
signed rational boundaries, paraboloids, reversed faces, multiple/disconnected
faces, joined faces, periodic cylinder seams, partial cylinder intervals,
singular natural faces, and parameter origins at `1e12`. They also cover Undo,
Redo, and repeated identical inputs with fresh owned object IDs. The 22 face
preselection cases cover individual and multiple faces, joined and disconnected
neighbors, reversed faces, natural no-ops, duplicate picks, mixed whole-object
selection, and the different ToEdge filter.

The independent source fixtures are
[base](../../tools/rhino_oracle/fixtures/shrink_trimmed_surfaces.json),
[history](../../tools/rhino_oracle/fixtures/shrink_trimmed_history.json), and
[additional geometry](../../tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json).
The [face fixture](../../tools/rhino_oracle/fixtures/shrink_trimmed_faces.json) adds
native preselection observations and component clearing through Undo/Redo.
Corresponding raw captures reside in `tools/rhino_oracle/observations/`.
Replay compares complete surface/curve definitions, raw numeric topology
indices, selection, identity, attributes, and memberships at absolute epsilon
`1e-9`, relative epsilon zero. Native renewal order varies between identical
runs, so snapshots are compared by source identity. Raw captures retain that
order; Viboceros renews changed objects in selection action order. Seven cropped
results also pass 3DM round-trip checks, including tight rotated trims, signed
weights, large parameter origins, shared topology, and partially cropped
polysurfaces with unchanged neighboring faces.

```sh
# Live capture always runs in a private Xvfb display.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.shrink_trimmed_capture tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json --timeout 300
python3 -m tools.rhino_oracle.shrink_trimmed_replay tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json tools/rhino_oracle/observations/shrink_trimmed_geometry.json
cargo test -p viboceros-geometry -p viboceros-document -p viboceros-command -p viboceros-oracle shrink --lib
cargo test -p viboceros shrink --bin viboceros
```

Application tests cover modifier clicks, toggles, face rectangles, cancellation,
mixed selection, stale geometry, and external Undo/Redo. Native mouse input is
not yet verified: the experimental
[picking fixture](../../tools/rhino_oracle/fixtures/shrink_trimmed_face_picking.json)
currently times out after delivering its first click under Wine. Failed runs
publish no observations and are not used as compatibility evidence. This remains
a verification gap for command-first face picking.

The stated cases do not establish behavior for every imported surface
representation or trim encoding. The geometry API exposes no side-disable mask.
The standard interval policy adapts the licensed OpenNURBS
`ON_Brep::ShrinkSurface` implementation; tight runtime UV bounds and `ToEdge`
behavior were independently implemented using public Rhino outputs and
[McNeel's command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/shrinktrimmedsrf.htm).
See [Rust adaptation provenance](../../third_party/opennurbs_rust/README.md).
