# Object snapping on block instances

Block instances now supply visible member geometry to the existing object-snap
algorithms. Results report the selectable root instance ID. Definitions and
instance relationships stay in the document; query records are read-only derived
data and create no objects, selection changes or history edits.
The [source provenance](block-snap-provenance.json) records the documented rule,
local adapter contract and verification evidence.

Point mode includes root and nested insertion points, following the
[Rhino object-snap documentation](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm).
References to empty nested definitions retain insertion-point targets when the
containing instance has supported geometry. Nested points follow child-to-parent
affine composition and their complete visibility path. There is no separate
insertion-point mode.

Member points/clouds, curve endpoints, Mid, Center, Quad, Near, mesh vertices and
supported intersections use the same kernels as independent objects. Mid-only
and curve-hover Center retain their existing admission and target policies.
Mesh Near/Mid/intersection wires require the existing mesh-wire switch; Vertex
remains independent. Intersections between different members of one instance
retain distinct geometric source identities while reporting the same root ID.

## Query records and caches

The adapter shares each placed member's immutable geometry snapshot with a
detached query record. Transient record IDs are distinct across members and
repeated placements. Lazy curve, polygon and mesh caches use those IDs; snap
results separately map their source to the owning root.

Unchanged instance snapshots reuse query records and feature caches across cursor
motion and viewport projections. Definition edits, transforms and unit changes
install new snapshots and refresh the derived records. Undo restores geometry
and invalidates the current records as necessary. Removed instances release
their records and obsolete feature data.

Visibility is resolved from root/member object flags and layers on each query.
Layer changes retain reusable geometry data. Locked geometry remains eligible,
matching the existing snap policy. Suspended queries return before source/cache
work. Documents without block definitions retain direct object iteration and do
not allocate a member-source vector.

Insertion-point collection retains the catalog's depth and traversal bounds;
empty branching graphs cannot cause unbounded traversal. The instance stores
these points with its other immutable derived data, so a snap query does not
recursively recompute placement matrices.

## Verification and boundaries

```sh
cargo test --release --workspace block
```

Drafting regressions cover every existing snap mode, nested/empty insertion
points, visibility/locking, independent placements, cache reuse, edit/Undo
refresh, removal and mesh/line intersections inside one root. CPU viewport tests
exercise Point and End in four views and mesh-wire admission through the drafting
cursor. These tests use the checked local geometry and prior snap algorithms;
no fresh native block-snap capture or GPU-backend run is claimed.

General occlusion culling, other Rhino snap modes and block subobject editing
remain incomplete. Structural 3DM round trips and linked/external insertion also
remain subject to their existing boundaries.
