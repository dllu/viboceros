# SubD Cap comparisons

[Cap usage](../commands/cap.md) · [Validation receipt](subd-cap-native-validation.json)

The public Rhino 8.32 command was captured on 26 independently authored control
cages, with complete input files, EndCommand observations, postscript state,
Undo and Redo. The two fixed matrices are
[subd_cap_regions](../../tools/rhino_oracle/fixtures/subd_cap_regions.json) (18)
and [subd_cap_holdouts](../../tools/rhino_oracle/fixtures/subd_cap_holdouts.json) (8).
Their raw observations and frozen producer sources are retained alongside them.
No observed output is used to construct a replay input.

The [world-metric follow-up](subd-cap-world-metric.md) adds 32 independently
authored holdouts, fixes oblique-plane distance distortion, and records the
remaining symmetric smooth-cap surface differences. This page and its original
validation record retain the scope and measurements of the first 26 cases.

Convex caps use a single face, or a triangle fan around the world bounding-box
center of each control loop. Triangular boundaries retain one triangle. Concave
and annular regions use the shared constrained triangulator. `Crease=No` smooths
joined boundaries; the public OpenNURBS tag update also reclassifies endpoints.
Original components, face colors, material channels, UVW coordinates, and sharpness
are retained by appending to the decoded archive. Closed inward results reverse
as a whole through the public solid-orientation query.

The tests compare counts, control positions, limit-vertex positions, oriented
face cycles, tagged edges, vertex tags, face properties/UVW where face cycles
correspond, and bidirectional samples of density-two limit meshes. Scene tests
also check attributes, layers, groups, selection, copies and history. The absolute
coordinate comparison bound remains `1e-6`; model tolerance is `1e-9`.
These are finite samples, not a certified bound over the entire limit surface.

The two annular cases retain different cap triangle diagonals and therefore
different interior edges. Face properties cannot be paired by matching face
cycles there; the diagnostic records that comparison as unavailable. Counts,
controls, limit vertices and sampled surfaces still meet the same bound. The
[retained gap list](../../tools/rhino_oracle/fixtures/subd_cap_retained_gaps.json)
is enforced in both command and application replays.

Two Native holdouts retain unusual behavior: a tilted loop's world bounding-box
center may lie off its plane, and a center lying on a boundary edge produces
only three nondegenerate fan triangles and leaves naked edges. These outputs
remain in the comparison matrix with their actual validity and topology.

Planarity is checked on every control-loop point. Coplanar source faces prevent
duplicate sheets. Intersecting or nonnested loops with overlapping bounds are
conservatively left open, including some disjoint shapes whose bounds overlap.
Shared constrained-region validation rejects touching or crossing hole loops.
An R-tree limits spatial candidates; containment children and nesting depths
are assembled once. Four million charged point comparisons bound planning;
OpenNURBS component and corner limits also apply. Errors leave source archives
and the complete command transaction unchanged.

Custom face-data retention has independent Rust tests. Native custom-material,
partial-UVW, arbitrary coincident-control and general self-intersecting SubD Cap
parity are not established. Release/Native comparative performance remains
unqualified. Existing mesh Cap and SubD editing limitations remain open.
