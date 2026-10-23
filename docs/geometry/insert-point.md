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
pairs more coplanar triangles first, then retains better standalone triangles.
New quads have strictly convex projected corners so boundary-origin changes
cannot create degenerate fixed facets. General-purpose quadrangulation ordering
is unchanged. Insertion
uses its validation tolerance so existing features below document tolerance
survive. Original n-gon membership follows adjacency through original edges and
their split segments. Quad origins follow the entire refined source boundary,
including splits between original corners; omitting them changes fixed diagonals.
New tessellation edges join automatic n-gons only for
regions with more than four boundary vertices; a four-corner region containing
a collinear inserted point retains separate triangle cells. Inserted chords
separate the resulting groups. This resolves both earlier n-gon grouping gaps.

The plan accepts an optional parallel-view normal for crossing projection;
otherwise it uses the first incident face normal. The session pins the first
supplied normal until all points are undone. A path is bounded to 256 picks and
one million source point/edge components. Native SubD limits also apply:
100,000 vertices/faces, 400,000 edges and 64 MiB serialized data.

`InsertSession` stages against an immutable geometry snapshot. Previews use a
separate document; Enter admits one transaction, while Cancel discards it.
Geometry, tolerance and editability changes invalidate admission. Typed input
is parsed completely and staged on a trial session before accepting the line.

The viewport chooses an original edge in screen space, then derives the free
point's depth from the original control display mesh using fixed 0-2 quad
facets. Rays outside that mesh use the camera-target plane. The closest 3D edge
parameter supplies the insertion and its cursor marker. SubD control meshes are
prepared lazily and shared across session clones. The original pick surface
survives local Undo and preview chords. Snaps take precedence over free depth.

The [kernel record](../insert-point-kernel-qualification.json) contains ten
independent SubD replays. The separate
[command record](../insert-point-command-qualification.json) covers 77 command
and 77 application getter replays plus 12 additional geometry cases. Replays use
saved source 3DM files and independent physical pixel rays. All original 65
command/getter workflows now agree under both surface-sampling policies at the
unchanged `1e-6` threshold; the two earlier warped depth failures are resolved.
The largest qualified coordinate or sampled surface error is about `2.35e-12`.
One planar repeated-edge primary case still differs in stored cells and facets.

Twelve new spatial scenes cover altered warp, aspect, winding, cyclic boundary
origin, reversed paths and repeated same-edge picks. Ten match geometry; one
planar scene differs in stored cells. The upper-left second-pick and same-edge
warped scenes remain unqualified under both sampling policies, with errors up
to `0.183`. The same-edge scene now completes instead of admitting a nearly
collinear quad that later fails validation. Both failures remain in the report,
with coordinates, channels, regions, preview isolation and history still checked.

Twelve dedicated n-gon scenes cover triangle overlays, repeated boundary picks,
reordered faces, triangulated fans, interior picks, crossing paths and adjacent
n-gons. Command and application replays compare boundaries, memberships,
coordinates, channels, preview isolation, metadata and document Undo/Redo.

Twelve independent shape scenes cover skewed, tall, rotated, reversed and warped
quads. Twenty warped Mesh/SubD depth scenes cover inside/outside rays, translated
sources, shifted camera targets, second picks and local Undo. New depth captures
record target/direction witnesses; older unchanged Top ZoomExtents captures use
the ray midpoint for target depth. Replays reconstruct surface hints from the
saved source and rays, without reading output geometry. Generic SDK `MeshRay`
hits use a shorter quad diagonal and remain separate calibration diagnostics.

Sixty public meshing API calibration recipes use captured *output*
regions. None reproduce all native InsertPoint layouts, including alternative
boundary origins, relaxed 3D quad conversion, line tessellation and planar meshing. These recipes do not
qualify command input behavior. An earlier failed holdout import retains its
request, host log and frozen sources; no native response survived.

Perspective point picking has local spatial-edge tests, but native perspective
crossing paths, changing views, constrained Osnap, grips, arbitrary spatial or
non-manifold topology, continuous surface error and comparative performance
remain unqualified. Full Rhino parity is not claimed.
