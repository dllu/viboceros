# Splop

Select objects or visible control/edit grips and run `Splop`. Pick the reference
sphere center, then enter its radius or pick a radius point. Enter accepts radius
1. Reference spheres also accept `2Point`, `3Point` (including a radius
constraint), `4Point` (including radius constraints), `Orientation`,
`AroundCurve`, and `FitPoints`. After picking a center, use `Diameter`,
`Circumference`, or `Area` to enter size. Area measures the reference circle,
so an area of `4*pi` gives radius 2. The construction plane supplies the default
normal; a picked radius supplies the source X direction. Select a destination
surface or face, pick its drop point, then drag the destination sphere to set
size and rotation together.

`FitPoints` accepts at least three point objects or visible curve/surface controls
and mesh vertex grips. `AroundCurve` asks for a curve or surface edge, projects
the center pick onto it, and builds the reference circle perpendicular to its
tangent. Both reuse the existing fit and circle-size getters and preserve the
guide geometry. The original transform sources stay fixed while guides are
picked. Unavailable curve guides return to curve selection.

In `4Point`, `Radius` after the second point completes a constrained base circle.
After the third point it constrains the sphere radius and asks which side of the
section plane contains its center. The radius must be at least the section
circle radius. Area input is a scalar in squared model units.

A numeric destination radius locks size while you continue picking direction.
Model-unit input is accepted in both radius prompts. `Copy=Yes` accepts repeated
placements from the original sources. Enter and Escape retain accepted copies
and preselected object/grip picks. Command-first picks are released. `FitPoints`
and `AroundCurve` clear the original selection when guide selection starts and
release guide picks when Splop ends. All accepted placements belong to one Undo
entry; cancelled reference input creates no edit and preserves Redo.
Copy defaults to Yes and uses the shared
`RememberCopyOptions` preference. Rigid and Flip start at No.

`Rigid=Yes` retains each object's shape while applying uniform sphere scaling
and placing it in the mapped local frame. Each selected top-level group acts as
one body. `Rigid=No` wraps the geometry through the plane-to-surface map. Selected
grips map individually, retaining their control or edit-point basis. Copying leaves
the displayed source grips at their original positions. Edited circular curves
retain their rational controls and expose derivative breaks as polycurve segments.

`Flip=Yes` reverses the placement's Y and normal directions and retains unit
source height while scaling the in-plane coordinates. A reversed target face
also changes orientation; its face sense does not itself disable height scaling.
The effective face orientation is read from the document.

Complete command input accepts numeric radii or radius points:

```text
Splop 0,0,0 2 0,0,5 4 Surface=<id> Copy=No
Splop 0,0,0 0,2,0 0,0,5 4,0,5 Surface=<id> Copy=Yes
Splop 0,0,0 2 0,0,5 4 Surface=<id> Face=0 Angle=45 Flip=Yes
```

Replace `<id>` with the destination object's ID. `Angle` is in degrees in the
effective target frame and is available with numeric destination radii. A final
point supplies both radius and direction. The target is excluded from source
edits. Invalid radii, unavailable references and failed fits leave geometry and
history intact. The Rust batch API can use `Finish=No` for accepted placements,
then `splop::finish_copies(doc, group, sources, postselected)` to finish their
shared history group. An explicit empty `GripSources=` excludes grips selected
for a reference getter from the captured transform sources.

The interactive controller shares surface/face picking and prepared display
cages with OrientOnSrf. Wrapped and rigid previews work in wireframe, shaded and
ghosted modes. Three great circles show each sphere during destination sizing.
Previews leave geometry and history unchanged; their approximations are separate
from the accepted tolerance-driven fits.

The [initial validation receipt](../splop-validation.json) records 134 native workflows,
replayed through the command API with both interpolation backends and through
the interactive controller. Captures cover planes, rational cylinders and
spheres, curves, single-face B-reps, boxes, meshes, point clouds, groups,
reversed faces, construction planes, radius locks, postselection, repeated copies
and cancellation. They compare geometry, names, identities, group memberships,
selection, unchanged guides and real Undo/Redo results.

Native fitted curves and surfaces use a `2e-5` comparison bound. Separate
off-grid checks constrain local fits to `1.1e-6` in these `1e-6` document-tolerance
fixtures. Mesh vertices compare exactly after conversion to Rhino's binary32
vertex storage. These are sampled geometry checks, not continuous deviation
certificates or performance measurements. Ill-conditioned or oversized fits can
exhaust the shared fitter and fail before document mutation.

The [grip and reference-sphere receipt](../splop-grips-reference-validation.json)
adds 148 native grip workflows and 21 reference-sphere workflows. Command and UI
replays compare complete rational control nets, knots, mesh vertices, grip display,
selection, groups and Undo/Redo. Grip numeric differences stay below `1e-9`;
reference-sphere cloud distances stay below `1e-9`. All 148 grip cases also check
preview owner controls against accepted geometry in the three display modes.

The [guided-reference and completion receipt](../splop-guided-validation.json)
adds 317 fitting, curve-guided and radius-constrained workflows and 48 true
Enter/Escape completion workflows. The current API and UI replay all 668 retained
workflows. Guided cloud comparisons use the `1e-6` document tolerance; unchanged
guides, grip bases and metadata use `1e-9` numeric comparisons. Noisy sphere fits
can differ slightly between the independent solvers.

The earlier copy captures sent an idle Cancel after Enter. Their selection
clearing reflects that second command, so the replay performs both actions.
The new completion captures omit the idle Cancel and verify that Enter preserves
preselected objects and grips. The earlier receipts are retained as historical
records. A separate input diagnostic retains the unsuccessful typed edge pick
and trailing-Cancel investigation.

Tangent remains in progress for the reference-sphere prompt. Native edge-input
and viewport pixel comparisons, continuous deviation certificates and Splop
performance measurements remain unqualified.

The retained dispatch diagnostic ran inside `RunPythonScript` and reported no
Undo records. It is separate from the validated corpus: Splop now runs at idle,
like the other owned command probes, and rejects a capture inside an active Undo
record. Every workflow with an accepted placement records `Undoing Splop` and
`Redoing Splop`; cancellation before placement has no Undo entry.

```sh
cargo test -p viboceros-command --test splop_command --test splop_grips --test splop_reference --test splop_guided --test splop_finish --config 'profile.test.package.viboceros-geometry.opt-level=2'
cargo test --bin viboceros splop --config 'profile.test.package.viboceros-geometry.opt-level=2'
python3 -m unittest tools.rhino_oracle.test_splop tools.rhino_oracle.test_splop_extensions tools.rhino_oracle.test_splop_guided
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_splop
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/splop.htm)
· [Public point-map API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.morphs.splopspacemorph/splopspacemorph)
