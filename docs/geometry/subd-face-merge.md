# SubD face merge observations

The SubD part of `MergeFaces` is being implemented. The mesh kernel is described
in [mesh face merging](mesh-face-merge.md); the application command remains
unregistered. This page records native behavior that the SubD kernel must match.

`subd_merge_faces` contains 41 actual Rhino command invocations: 33 geometry
changes, seven successful unchanged results, and one empty-selection cancellation.
The capture records control points, tags, oriented edges, face loops, component
IDs, limit points and a sampled limit mesh. Before/after 3DM archives retain
properties such as stored edge sharpness for subsequent public OpenNURBS readback.

## Observed behavior

- Merging also removes some vertices that become valence two. Two quads in a
  strip become a four-vertex face; two triangles in the square fixture become a
  three-vertex face. Nonplanar control nets exhibit the same topology cleanup.
- In these fixtures, merged faces reuse the smallest contributing face ID.
  Untouched faces retain IDs; the reused face can move to the end of native
  iteration order. Edge IDs survive endpoint rewiring during cleanup.
- Hard creases are not unconditional merge barriers. Selecting the shared hard
  edge of two quads leaves a six-vertex face and retains corner tags. Selecting
  both faces or a shared endpoint removes the two intermediate vertices instead.
  The captured soft-crease edge follows the ordinary four-vertex cleanup.
- Disconnected regions merge independently. Regions with a hole, or an entire
  closed shell with no boundary, remain unchanged. Partial rings can merge.
- Changed results retain attributes and select result faces. A surviving input
  vertex remains selected. Unchanged results clear component selections and add
  no Undo entry: Undo reaches an earlier, independently authored point witness.
  Changed results undo and redo exactly, including component IDs and limit data.

## Verification and remaining work

```sh
python3 -m unittest tools.rhino_oracle.test_subd_merge_faces
```

The tests check bounded recipes, private capture isolation, content-addressed
producer provenance, component tables, attributes, history and the observations
above. These are native reference tests, not kernel parity results. Stored
sharpness readback, public topology implementation, broader region qualification,
`Results`, getters and application selection/history remain to be completed.
`input_timer_fired` records timer activity and does not establish getter opening.
