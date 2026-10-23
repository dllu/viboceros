# InsertPoint geometry and staging

[Command usage](../commands/insert-point.md) · [Geometry guides](README.md)

`SubD::try_insert_edge_path` and `TriangleMesh::try_insert_edge_path` accept
ordered normalized picks on original topology edges. A shared plan inserts
explicit points before automatic crossings and connects consecutive picks
through incident faces. Disconnected components receive their own points
without an invented bridge. Crossing inserted chords return an error.

SubD edits use public OpenNURBS `SplitEdge` and face-local-index `SplitFace`
operations in one archive decode/edit/encode, retaining existing tags and
component identities. Near-endpoint user parameters clamp to `[0.001, 0.999]`.
Meshes keep raw seams separate, interpolate color/UV/normal channels, and reuse
the existing constrained triangulation and quadrangulation code. Mesh geometry
uses its validation tolerance so existing features below document tolerance
survive. Original logical n-gons are retained or divided along inserted chords;
two captured native n-gon grouping choices still differ.

The plan accepts an optional parallel-view normal for crossing projection;
otherwise it uses the first incident face normal. The session pins the first
supplied normal until all points are undone. A path is bounded to 256 picks and
one million source point/edge components. Native SubD limits also apply:
100,000 vertices/faces, 400,000 edges and 64 MiB serialized data.

`InsertSession` stages against an immutable geometry snapshot. Previews use a
separate document; Enter admits one transaction, while Cancel discards it.
Geometry, tolerance and editability changes invalidate admission. Typed input
is parsed completely and staged on a trial session before accepting the line.

The [kernel record](../insert-point-kernel-qualification.json) contains ten
independent SubD replays. The separate
[command record](../insert-point-command-qualification.json) covers 21 command
and 21 application getter replays plus 12 spatial, triangular, seam, n-gon and
original-edge cases. Replays use saved source 3DM files and independently
captured physical pixel rays. The largest coordinate or sampled surface
difference is about `2.35e-12` at a `1e-6` acceptance threshold. Three primary mesh cases differ
in their stored triangle/quad cells; extra holdouts retain two grouping and
two cell-layout differences. These are explicit diagnostics.

Thirty-two public meshing API calibration recipes use captured *output*
regions. Neither public triangulation nor quad conversion, across every cyclic
boundary origin, reproduces all native InsertPoint layouts. These recipes do
not qualify command input behavior. An earlier failed holdout import retains
its request, host log and frozen sources; no native response survived.

Perspective point picking has local spatial-edge tests, but native perspective
crossing paths, changing views, constrained Osnap, grips, arbitrary spatial or
non-manifold topology, continuous surface error and comparative performance
remain unqualified. Full Rhino parity is not claimed.
