# ScaleByPlane

[Command reference](README.md) · [Transforms](transforms.md)

`ScaleByPlane` scales selected geometry independently along a chosen plane's X
and Y axes. It retains offsets along the plane normal. Pick a scaling origin,
first reference point, and second reference point. Each axis uses the signed
ratio of the second reference's component to the first reference's component,
measured from the scaling origin. Negative ratios reflect that axis.

If either component has magnitude at most `1e-6`, that axis stays unchanged.
This includes a reference at the origin and a target component of zero; these
inputs do not flatten the geometry. The cutoff is inclusive, as measured with
the exact value and adjacent binary64 values in Rhino.

```text
ScaleByPlane 1,2,3 3,5,3 5,11,3
ScaleByPlane Plane=WorldFront 1,2,3 3,2,6 5,2,12 Copy=Yes
ScaleByPlane Plane=3Point 0,0,0 1,1,0 -1,1,2 1,2,3 3,5,3 5,11,3
ScaleByPlane Plane=Object object-id 1,2,3 3,5,3 5,11,3
```

Complete invocations use world coordinates. Interactive typed points use the
usual construction-plane input rules; prefix `w` for world coordinates.

## Plane choices

Choose `Plane=...` before the scaling origin. Typing `Plane` opens the choices.
The plane starts at `ActiveCPlane` for each command.

| Choice | Reference axes |
| --- | --- |
| `ActiveCPlane` | Active viewport's construction plane |
| `WorldTop` | World X and Y |
| `WorldFront` | World X and Z |
| `WorldRight` | World Y and Z |
| `3Point` | Plane origin, X direction point, orientation point |
| `Object` | Selectable planar curve, surface, or B-rep face; complete input accepts an object ID and optional `Face=index` |
| `FromView` | Click a viewport; Enter chooses the active viewport |

The chosen plane's axes are retained through reference picks and repeated
copies. Its stored origin does not replace the scaling origin. Picking an Object
target does not add it to the transform sources. Planar surface U/V axes supply
its frame;
planar curves use the existing object-frame resolver. Nonplanar targets are
rejected without applying a transform.

The [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/scale.htm#ScaleByPlane)
describes `FromView` as choosing a view plane. In the captured Rhino build,
independent comparisons of Top, Front, and Perspective with world and tilted
construction planes show that its effective scale axes follow the clicked
viewport's construction plane, rather than its camera frame. Viboceros follows
that measured behavior. A complete `FromView` invocation uses the construction
plane provided by the command context.

## Copy, Rigid, and control points

Start without selected objects to [pick sources first](transform-sources.md).
Displayed selected grips take precedence over a selected parent: ordinary
scaling edits those controls, and Copy duplicates their owner with only the
selected controls changed. Curve knots and weights, surface parameters, and
mesh face indices are preserved by the shared affine edit path.

`Copy=Yes` retains the original sources, origin, and first reference. Keep
entering second reference points to make additional copies, then press Enter
or Escape. Accepted copies share one Undo entry.
[RememberCopyOptions](remember-copy-options.md) governs the Copy default.

`Rigid=Yes` moves each object's tight bounding-box center through the scale map
while preserving its shape. Selected group members share their group's bounds,
using the existing [rigid placement rules](scale-nu.md#rigid-placement).
Rigid can change during point input and is remembered by Viboceros, including
after cancel.
It starts at No in a fresh registry. A bare `Rigid` toggles it.

Rigid excludes selected grip owners. Their geometry stays unchanged. Rhino can
retain temporary grip display locations from the last cursor position after a
typed completion; Viboceros does not reproduce that unprescribed display state.
It keeps their original display locations. This is a known compatibility gap.

## Verification and limits

The [capture and replay notes](../scale-by-plane.md) describe 106 actual public
Rhino commands and 212 complete/incremental application replays. Comparisons
cover geometry definitions, object names, selected controls, object order,
Copy, selection cleanup, and external Undo/Redo at absolute epsilon `1e-9`.
The three Rigid grip recipes compare geometry and history while excluding the
cursor-dependent display coordinates.

The application provides live affine previews using the same reference map as
execution. Native preview appearance, prescribed mouse reference workflows,
SubCrv input, Object frames for arbitrary curves and trimmed faces, unusual
group layouts, extreme coordinates, and performance parity remain unverified.
Mesh Object targets are not implemented.
These captures do not establish complete Rhino compatibility.
