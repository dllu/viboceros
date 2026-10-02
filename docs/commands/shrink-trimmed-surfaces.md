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

Knot insertion and surface cropping retain the visible trim image without
fitting. UV coordinates, orientation, vertices, spatial edges, trim curves
(including their weights), topology indices, object IDs, attributes, and groups
are preserved. Underlying surface knots and control nets are cropped. Cached isoparametric classes are updated for the new domains.
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

## Verification and limits

132 saved Rhino 8.32 command cases cover both selection workflows, rectangular
and curved trims, holes, rotated circles and ellipses, cubic polynomial and
signed rational boundaries, paraboloids, reversed faces, multiple/disconnected
faces, joined faces, periodic cylinder seams, partial cylinder intervals,
singular natural faces, and parameter origins at `1e12`. They also cover Undo,
Redo, and repeated identical inputs with fresh owned object IDs.

The independent source fixtures are
[base](../../tools/rhino_oracle/fixtures/shrink_trimmed_surfaces.json),
[history](../../tools/rhino_oracle/fixtures/shrink_trimmed_history.json), and
[additional geometry](../../tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json).
Corresponding raw captures reside in `tools/rhino_oracle/observations/`.
Replay compares complete surface/curve definitions, raw numeric topology
indices, selection, identity, attributes, and memberships at absolute epsilon
`1e-9`, relative epsilon zero. Native renewal order varies between identical
runs, so snapshots are compared by source identity. Raw captures retain that
order; Viboceros renews changed objects in selection action order. Five cropped
results also pass 3DM round-trip checks, including tight rotated trims, signed
weights, large parameter origins, and shared topology.

```sh
# Live capture always runs in a private Xvfb display.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.shrink_trimmed_capture tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json --timeout 300
python3 -m tools.rhino_oracle.shrink_trimmed_replay tools/rhino_oracle/fixtures/shrink_trimmed_geometry.json tools/rhino_oracle/observations/shrink_trimmed_geometry.json
cargo test -p viboceros-geometry -p viboceros-document -p viboceros-command -p viboceros-oracle shrink --lib
cargo test -p viboceros shrink_commands_route --bin viboceros
```

Face subobject picking is not implemented. The stated
cases do not establish behavior for every imported surface representation or
trim encoding. The geometry API currently shrinks all faces and exposes no
side-disable mask. The standard interval policy adapts the licensed OpenNURBS
`ON_Brep::ShrinkSurface` implementation; tight runtime UV bounds and `ToEdge`
behavior were independently implemented using public Rhino outputs and
[McNeel's command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/shrinktrimmedsrf.htm).
See [Rust adaptation provenance](../../third_party/opennurbs_rust/README.md).
