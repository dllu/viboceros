# Surface Inset

[Inset](inset.md) · [Command input](../command-line.md)

`Inset` partitions selected planar faces into an inset cap and border
faces. Select a surface or polysurface, select individual faces, or start the
command and click/window faces, then press Enter. Enter a positive distance to
preview the result, then Enter to commit. Escape cancels without changing geometry
or remembered settings. Surface distance starts at 1 and is remembered separately
from mesh and SubD distances.

```text
Inset 0.3
Inset 0.3 Faces=0,1 CreaseCorners=Yes
```

`CreaseCorners=No` (initial default) creates one annular border face per boundary.
`Yes` connects old and inset corners, creating one strip per edge. Outer boundaries
shrink; holes expand. Concave polygons and adjacent selected faces are supported
when the offset retains the boundary topology. Whole surface inputs address all
faces; typed `Faces` lists restrict the selected objects to zero-based face indices.
Surface inputs must be processed separately from mesh/SubD inputs.
Smooth NURBS loops, circular holes, annuli and tangent-continuous line/arc loops
are also supported; see [curved-boundary behavior and qualification](inset-curves.md).
A single smooth closed edge stays annular even with `CreaseCorners=Yes`.

The operation replaces each input while retaining its ID, attributes, ordered
groups and whole-object selection. Surface component picks clear after completion.
The supporting NURBS surfaces, old vertices/edges and unselected faces are retained;
new edges are shared between caps and borders. Selected face slots contain caps,
and border faces are appended. Native Rhino face/edge table order and parameter
clocks can differ. One edit supports Undo/Redo. Stale geometry, selection, metadata
or tolerance rejects the preview before any change.

This path requires affine planar charts. Polygon loops use individual degree-one,
two-control-point edges and trims; curved loops require smooth spans and joins.
Higher-degree polygon-edge encodings, curved G0 corners, nonplanar faces, seams,
poles, collapsed edges and
crossing/touching inset loops return errors. Insets that change topology at the
medial axis are also rejected. `ThroughPoint`, `Loose`, `IgnoreSeams`, `Split=No`
and `JoinOutputs=No` remain pending; output currently stays joined to the input.

Frozen private-display evidence includes 94 public `Brep.InsetFaces` cases and
90 commands covering outputs, attributes, selection and real history. The kernel
matches 41 polygon and 34 curved SDK cases plus 30 joined command cases; pending
nonplanar and loose-fit results remain in the immutable evidence. Polygon and
analytic-curve comparisons use `1e-10`; fitted-curve tolerances are documented in
the [curved-boundary guide](inset-curves.md). Checks retain oriented loops, edge
incidence, supporting surfaces and solid status.
Controls include triangles, reflected face sense, skewed charts and subdivided
collinear boundaries. Collinear inset corners simplify; their old crease
connectors meet the following retained corner, matching the captured native strips.
Joined outputs also roundtrip through native 3DM. App tests cover preselection,
command-first input, previews, cancellation, history and all three display modes.

Kernel admission allows 100,000 selected corners, 16 million boundary-pair work
units and a conservative 64 MiB output estimate. Supporting surface copies are
charged before construction, including refined planes. Command staging and preview
limits are shared with [mesh/SubD Inset](inset.md).

```sh
cargo test -p viboceros-geometry --lib planar_inset
cargo test -p viboceros-command --lib inset
cargo test -p viboceros-oracle --test inset_surface
cargo test -p viboceros --bin viboceros inset
python3 -m unittest tools.rhino_oracle.test_inset_surface
```
