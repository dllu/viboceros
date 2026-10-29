# Component-selected SubD face merging

`SubD::merge_face_components` merges connected face regions using public
OpenNURBS topology tools on an owned archive copy. `SubDMergeComponent` accepts
original vertex/face ordinals or two control-point ordinals identifying an edge.
The result includes the number of removed faces and rewritten result-face
ordinals. The [MergeFaces command](../commands/merge-faces.md) uses this kernel.

Native vertex, edge and face IDs are available alongside the control tables,
which are sorted by ID. These identities survive editing and archive readback;
ordinals can change after components disappear. Archive serialization normalizes
legacy maximum-ID fields. [Sequential edit validation](subd-edit-history.md)
checks actual global/local SubDivide allocation after merging, including public
SDK and archive copies. Existing and allocated component ID tables match exactly
in that corpus; other allocating edits remain unqualified.

## Region and cleanup behavior

Overlapping explicit face requests take precedence over edges, then accepted
edges take precedence over vertices. An edge ignored because it touches a picked
face does not consume an independent vertex request. Singleton face requests do
not recruit neighboring faces. Disconnected regions merge independently; a new
region needs one simple oriented boundary. Holes and closed regions without a
boundary remain unchanged.

Merging removes interior components and simplifies eligible valence-two vertices.
Boundary junctions of original valence four or greater remain, including
collinear junctions. Interior vertices can be removed at higher original valence.
Every incident face must keep at least three edges. Face and vertex requests can
remove an affected corner; an edge request retains corners. Corners that survive
keep their tags. Accepted explicit vertex requests can also simplify a separate
single-face region when another region actually merges.

Merged regions reuse their smallest face ID. Surviving edge IDs remain stable
when endpoints change; stored sharpness follows the retained edge. Unselected
faces keep their records, though shared boundary cleanup can alter their loops.
Empty input returns the original value. Invalid indices and native failures leave
the source unchanged. Component release is local to each region, with one final
update of tags and evaluation caches, avoiding repeated whole-model scans.

## Qualification

The [validation receipt](subd-face-merge-validation.json) binds the current Rust
and C++ bodies and four immutable native reference families, totaling 94 actual
Rhino command invocations. They cover face, edge and vertex requests, mixed
priority, corners, hard and soft creases, twisted control nets, winding changes,
holes, partial rings, disconnected regions and cleanup at different valences.

All 188 before/after native 3DM archives are checked against captured control nets,
IDs, tags, face loops and limit points. Kernel replays compare the full definition,
stored sharpness, rewritten face IDs and sampled limit-surface facets. Continuous
coordinates use `1e-7`; discrete properties and stored sharpness are exact. The
same audit runs in a binary that explicitly initializes SMLib. Regression tests
also cover invalid selections, rejected regions beside valid ones and 300
independent regions.

```sh
cargo test -p viboceros-geometry --lib subd::
cargo test -p viboceros-io --test subd_face_merge
cargo test -p viboceros-io --test subd_face_merge --features native-smlib
python3 -m unittest tools.rhino_oracle.test_subd_merge_faces tools.rhino_oracle.test_subd_merge_kernel
```

Native selection/history observations also guide the application command.
Unchanged commands clear component selections and add no Undo entry; changed
commands preserve attributes and undo/redo exactly. `input_timer_fired` records
timer activity and does not establish getter opening.

Other allocating edits, nonmanifold regions, material merging and custom face
textures remain outside the qualified corpus. Multiple owners, Results and application
selection/history are covered by the separate command receipt.

[Per-face colors](subd-face-properties.md) have a separate native corpus:
merged faces clear their colors while untouched and unchanged faces retain them,
including sequential merges and Undo/Redo.
