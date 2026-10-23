# InsertPoint kernel foundation

`SubD::try_insert_edge_path` inserts ordered normalized points on original
control edges and connects them through incident faces. It uses the public
OpenNURBS `SplitEdge` and face-local-index `SplitFace` operations in one native
archive edit. Existing tags and component identities remain in the archive.
Explicit points precede automatically inserted crossings on intermediate edges.
User edge parameters near an endpoint clamp to `[0.001, 0.999]`.

The API accepts an optional parallel-view normal for crossing projection.
Without one, it uses the first incident face's normal. It bounds a path to
256 picks and the source to one million combined point/edge components.
Invalid input returns an error without modifying the source archive.
Existing archive limits also apply: 100,000 vertices/faces, 400,000 edges
and 64 MiB of serialized data.

The [qualification report](../insert-point-kernel-qualification.json) retains
45 native workflows, with ten independent SubD kernel replays. The replays
start from exact saved 3DM sources and use physical pixel-ray and projected-point
witnesses from an owned Rhino Top viewport. Control ordinals, topology, tags,
limit stations and sampled density-two facets agree within `1e-6`; the largest
sampled difference is approximately `1.38e-14`. Five separate tests cover
invalid late inputs/native chords, unchanged source archives, disconnected
components, rotated/translated paths, existing-vertex crossings, repeated
points, no-ops and resource limits.

`InsertPoint` is not registered as a command yet. Mesh n-gon tessellation and
quad origins remain under implementation. The captured mesh, typed-coordinate
discovery and early physical inputs remain excluded from kernel qualification.
One later physical input timed out inside its getter; its trace remains intact.
Document history, local Undo, cancellation, grips, perspective projection,
intersecting path chords, arbitrary spatial topology, continuous surface error
and comparative performance remain unqualified.
