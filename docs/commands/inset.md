# Inset

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

`Inset` inserts border faces inside planar surfaces, mesh polygons or SubD control
faces. [Surface inputs and limitations](inset-surfaces.md) have a separate guide.
For mesh and SubD inputs, select faces before starting, or start the command and click or window
faces, then press Enter. Ctrl/Command removes picks. Enter a positive distance
to update the control-cage preview, then Enter to commit; Escape cancels.

```text
Inset
Inset 0.3 Faces=0,1 Mode=Group
Inset 0.3 Faces=All Mode=Single
```

Typed `Faces` selectors address zero-based faces on selected objects; the
interactive getter uses component picks. Entire objects alone do not select
faces. Whole planar surface inputs also support implicit face selection.

SubD Group mode surrounds the combined face boundary. Internal controls stay in
place; boundary controls follow available internal edges at folds. Single mode
insets each face independently. One selected SubD face uses the individual-face
layout. Mesh faces are always individual. Mesh picking addresses stored triangles
and quads, including members of n-gons. Both modes retain original cap-face indices.
Grouped closed nets without a boundary, and branching selected boundaries, are
unchanged, matching the measured native cases.

Distances initially default to 0.1 for SubDs and 1 for meshes. Each type remembers
its last completed distance outside model history; cancellation keeps that value.
Mode starts at Group on every invocation. Nonpositive distances reprompt without
replacing the staged value. New cap faces remain selected after completion;
`SelNone`, Escape, and history replay release the transient component picks.
One modeling edit preserves input IDs, layers, names, colors and ordered groups.
Changed inputs, selection, attributes, groups or tolerance reject a stale preview.

Mesh normals are rebuilt and new texture coordinates are zero-filled. Native
mesh insets leave incomplete color arrays and stale n-gon records in some cases;
this model discards those incomplete channels and invalid logical overlays while
retaining the measured vertices and faces. SubD insets that collapse into an
invalid native cage are rejected. Large positive distances are otherwise allowed,
including inverted shapes; self-intersections are not repaired.

Frozen private-display captures cover 61 SubD cases, ten additional input/control
cases, 112 mesh geometry/getter cases and a seven-command preference sequence.
Geometry replay compares controls, raw face/edge order, tags, vertex limit stations,
mesh normals and UVs at `1e-10`. Metadata, cap selection and actual Undo/Redo are
checked separately. Native macro cleanup clears cap picks after EndCommand; app
tests explicitly apply that separate cleanup before replaying history. Pointer
and window tests cover wireframe, shaded and ghosted modes, including n-gon members.
Refined SubDs, soft sharpness and mesh outputs roundtrip through native 3DM.

Rhino's scripted mesh getter can commit its previous preview distance after a
typed change. The saved getter traces retain that behavior, including selection retained after
an extra numeric token aborts a macro as an unknown command. Geometry captures
also use completed edits on disposable reference meshes to establish the requested
default before measuring unchanged test sources. The app updates its staged
preview directly; physical native mouse/getter timing remains unqualified.

SubDs are bounded to 100,000 controls/faces, 400,000 edges/corners and 64 MiB per
archive. Individual-face connection work is limited to 16 million incident-face
visits. Mesh results allow four million vertices and one million faces. Commands
bound aggregate staged data to 64 MiB, one million input targets and previews to
400,000 segments. Two-point distance entry, face-loop double-click selection,
per-face materials, texture/packing parity and comparative speed need further work.

```sh
cargo test -p viboceros-geometry --lib inset
cargo test -p viboceros-command --lib inset
cargo test -p viboceros-oracle --test inset
cargo test -p viboceros --bin viboceros inset
python3 -m unittest tools.rhino_oracle.test_inset
```

Reference: [Rhino Inset](https://docs.mcneel.com/rhino/8/help/en-us/commands/inset.htm).
