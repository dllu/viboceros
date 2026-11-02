# Direction edits

[Analysis commands](analysis-index.md) · [Flip](flip.md)

```text
Dir
Dir Flip|UReverse|VReverse|SwapUV [Faces=All|i,j]
Dir Mode=FlipNormal|FlipU|FlipV|SwapUV [Faces=All|i,j]
```

Bare `Dir` inspects selected objects, or asks for objects and Enter first.
`Direction` is an alias. Viewports show curve tangents, surface U/V axes and
oriented normals, and mesh face normals (up to 256 sampled triangles). Hovering
a single object moves its arrows to the cursor. End-on arrows use a dot toward
the camera and a cross away. Click an inspected object to preview a reversal.
Navigation and display-mode changes remain available during inspection.

Type `Flip`, `UReverse`, `VReverse` or `SwapUV` for repeated provisional edits.
`Mode=FlipNormal|FlipU|FlipV|SwapUV` sets the multiple-object click mode;
`FlipAll` applies that mode to all inspected objects. `NextMode` cycles
FlipU → FlipV → FlipNormal → SwapUV. The mode persists between commands.
Enter accepts the final geometry in one Undo entry; Escape discards all edits.
Inspection and edits that return to the original geometry preserve history and
Redo. Source changes invalidate the preview. `ShowDir` and `ShowDirOff`, arrow
appearance preferences and parameter/draft-angle status readouts are not implemented.

With an explicit edit on the initial command line, `Dir` executes immediately
on preselection, or after picking objects and Enter.

`UReverse`, `VReverse` and `SwapUV` edit a surface's parameter directions without
changing its spatial geometry or oriented normal. Curves and meshes in a UV-edit
selection are errors; the complete batch is prepared before any edit is admitted.
A single-face B-rep needs no selector. `Faces=0,2` or `Faces=All` explicitly applies
a UV edit to selected polysurface faces; other faces and all spatial edges remain
unchanged. This face-list syntax is a Viboceros batching extension, qualified
against the public face API separately from actual `Dir` commands. Face lists
are not accepted with normal flipping.

`Flip` reverses curve traversal, mesh winding and open surface/B-rep face sense.
Closed B-rep solids and unsupported peers such as points are skipped. See
[Flip's supported geometry](flip.md). UV edits preserve solid orientation, so
a closed surface's parameter directions remain editable even when its normal
cannot be flipped. A standalone NURBS surface becomes a single-face B-rep to
store its independent face sense; Undo restores the original representation.

Objects retain their identities, attributes and groups. Explicit single edits
and single-object inspection retain preselection after execution and Redo;
Undo clears changed inputs. Multiple-object inspection retains preselection
through Undo too, but clears it on Escape. Command-first inspected inputs are
deselected on acceptance or Escape and remain deselected through Undo/Redo.
Invalid edits preserve the previous preview. Scripted all-skipped batches add
no Undo entry and preserve Redo and skipped peers' selection.

## Geometry and validation

`brep/parameter_orientation` owns the covariant chart edits. Surface knots and
trim coordinates change together: a reversed coordinate is negated and domain
`[a,b]` becomes `[-b,-a]`. Every edit reverses loop traversal and compensates face
sense; trim edge-use directions and iso labels follow the new chart. SwapUV also
exchanges UV tolerances. Spatial edges, vertices, curve parameters and shared
incidence remain unchanged. Native extrusion caches are invalidated.

Eight unedited shared 3DM operands cover holed and reversed planar faces,
asymmetric rational surfaces, cylinder seams, sphere poles, a cone apex, a torus
and a six-face box. Frozen Rhino 8.32 captures retain complete definitions,
weights, knots, topology, tolerances, 81 surface samples per face and 33 points
per edge and lifted trim, plus document snapshots through Undo/Redo.

The [52 named command cases](../../tools/rhino_oracle/fixtures/brep_direction_command.json)
cover seven single-face operands with preselection and command-first selection.
A separate [30 public face-API cases](../../tools/rhino_oracle/fixtures/brep_direction_sdk.json)
include one, several and all box faces. SDK replacement retains selection through
Undo; it is a separate execution route, not evidence of `Dir`'s selection policy.
Another [36 single-object sessions](../../tools/rhino_oracle/fixtures/brep_direction_sessions.json)
and [32 paired-surface mode sessions](../../tools/rhino_oracle/fixtures/brep_direction_modes_seeded.json)
cover repeated edits, no-ops, Enter, Escape, both selection routes and mode
cycling. Paired recipes explicitly seed the persistent mode. Both source
identities and complete model states are checked; rendered arrow pixels and
multiple-curve/mesh selection policies have no native differential capture yet.
Compressed raw responses and content-addressed producer snapshots remain under
`tools/rhino_oracle/observations/`. Current portable replays compare every model
field at absolute `1e-9`, with exact topology and selection. This is bounded
corpus evidence, not a general performance or continuous-error certificate.

```sh
cargo test -p viboceros-geometry --lib brep::parameter_orientation
cargo test -p viboceros-command --lib tests::surfaces::direction
cargo test -p viboceros-command --lib direction::session
cargo test -p viboceros-oracle --test brep_direction_native
cargo test -p viboceros --bin viboceros app::tests::direction
cargo test -p viboceros --bin viboceros viewport::direction
python3 -m unittest tools.rhino_oracle.test_brep_direction
```

Public references: [Dir options](https://docs.mcneel.com/rhino/8/help/en-us/commands/dir.htm),
[BrepFace Reverse and Transpose](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.brepface/).
