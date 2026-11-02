# Direction edits

[Analysis commands](analysis-index.md) · [Flip](flip.md)

```text
Dir Flip|UReverse|VReverse|SwapUV [Faces=All|i,j]
Dir Mode=FlipNormal|FlipU|FlipV|SwapUV [Faces=All|i,j]
```

Run a specified edit on selected objects, or start it before picking objects and
press Enter to accept. `Direction` is an alias. Bare `Dir` direction inspection,
arrows, live mode switching and click-to-flip remain unimplemented.

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

Objects retain their identities, attributes and groups. Preselection remains
selected after execution and Redo; Undo clears changed inputs. Command-first
picks are deselected after execution, Undo and Redo. Skipped peers retain their
selection. Invalid edits preserve geometry, selection and history; an all-skipped
batch adds no Undo entry and preserves Redo.

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
Compressed raw responses and content-addressed producer snapshots remain under
`tools/rhino_oracle/observations/`. Current portable replays compare every model
field at absolute `1e-9`, with exact topology and selection. This is bounded
corpus evidence, not a general performance or continuous-error certificate.

```sh
cargo test -p viboceros-geometry --lib brep::parameter_orientation
cargo test -p viboceros-command --lib tests::surfaces::direction
cargo test -p viboceros-oracle --test brep_direction_native
cargo test -p viboceros --bin viboceros app::tests::direction
python3 -m unittest tools.rhino_oracle.test_brep_direction
```

Public references: [Dir options](https://docs.mcneel.com/rhino/8/help/en-us/commands/dir.htm),
[BrepFace Reverse and Transpose](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.brepface/).
