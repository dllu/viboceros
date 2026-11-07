# BlendCrv surface edges

[Command guide](commands/blend-curve.md) · [Validation record](blend-curve-edges-validation.json)

`Edges` selects supporting-face constraints at the clicked edge locations.
`Curves` selects ordinary curve constraints, including boundary curves. Native
captures establish this distinction: the same plane boundary produces a tangent
along the boundary in Curves mode and across the face in Edges mode.

Surface constraints use oriented boundary tangents, regular surface normals and
normal curvature from the surface shape operator. This supports G0–G2 on regular
faces and G3/G4 on planar faces. `AtAngle_1/2` changes tangent angle in degrees,
preserving physical handle lengths. Contact slides, shape edits and Shift
pairing share the existing transient editor. Surface parents never supply join
pieces; mixed ordinary curves retain their trim/join identity and attributes.

Four private Xvfb captures retain 48 native observations. Both portable and
usd-brep builds replay 41 complete workflows, including 40 accepted blends and
one cancellation preserving pending redo. Seven observations remain diagnostics:
four selector discoveries with rejected options, one extra-Enter cancellation
probe, and two complete curved-face G3/G4 observations. One failed passive API
capture is retained with its exact producers. A failed replay assumption is also
retained: Perspective picks may have several frustum hits, so replay requires a
unique nearest screen hit while preserving every candidate.

The matching workflows include plane, cubic, rational and nonorthogonal surface
parents, Top/Perspective views, mixed sources, angles, contact/shape/Shift edits,
Flip, trimming/joining, source identities, undo/redo and 82 command/editor 3DM
round trips per build. Geometry and normalized derivative gates are `1e-6`;
domain precision uses the previously qualified `max(2e-6, 1e-8 * native span)`.
Raw coefficients, finite samples, derivatives and absolute errors are retained.

Prescribed surface definitions construct the Rust scenes. Passive public
PickContext receipts record edge candidates, parameters, screen distances,
supporting-face UVs/normals and trim orientations before input. Output geometry
is used only as an expectation. No desktop window is opened.

Curved-face G3/G4 is explicitly rejected. Alt dragging, shared-face selection,
seams, trimmed/closed boundaries, singular frames, arbitrary overlapping picks,
continuous-distance proof and comparative performance remain unqualified.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_edges
```
