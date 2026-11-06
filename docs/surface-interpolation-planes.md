# InterpCrvOnSrf plane input

At surface selection, type `IP`, `IPlane` or `InfinitePlane`. Choose World or
CPlane axes, `ThroughPoint`, `3Point`, `Vertical`, a planar object/face,
`AroundCurve` or a saved `NamedCPlane`. The shared picker also offers Rectangle.
Vertical needs two defining points. Enter or Escape abandons plane definition
and returns to surface selection; another Escape cancels the command.

Accepted planes retain their frame without changing the viewport CPlane.
Point Undo revises the draft, and Enter or Close commits one history edit.
Close requires three distinct locations. Output curves have fresh attributes
on the current layer and remain unselected. Input geometry, groups and pending
Redo survive cancellation; changed references or tolerance invalidate a draft.

Headless commands accept the same plane definitions as Project:

```text
InterpCrvOnSrf -4,-3,0 -2,2,0 0,-1.5,0 2,3,0 InfinitePlane=WorldXY
InterpCrvOnSrf 3,-4,0 3,0,2 3,4,0 InfinitePlane=AroundCurve:curve-id:parameter
InterpCrvOnSrf 0,0,4 2,3,4 4,1,4 InfinitePlane=Object:object-id
```

Rhino's input is a **finite pseudo-infinite patch**. Project the document's box
corners into the selected frame and grow each interval by `0.01*(length+1)`.
Input points clamp to that footprint. An empty model therefore gives intervals
of `[-0.01,0.01]` around the projected world origin; distant inputs may collapse
to one corner. The resulting cubic can overshoot the input footprint.

The command captures this support once and creates no temporary model surface.
The geometry crate supplies independent ideal-plane interpolation, exact private
support extension and its existing surface-image certificate. Separate common-sign
control-hull and interpolation-station checks keep the result on the requested
plane within the model tolerance.

The [validation record](surface-interpolation-planes-validation.json) retains
43 native observations across five immutable capture families. The 23 qualified
UI workflow replays include 18 constructions and five cancellation/invalid-input
controls in both builds, with 3DM round trips. The 18 constructions also replay
through the headless command API. Whole-span bidirectional
samples retain the `1e-6` gate; the empty-document comparison is about `9.2e-7`.
Exploratory failures with a distant line are retained separately. Three early
discovery cancellations needed recorded external recovery and are excluded from
the qualified workflows. Later cancellation captures bind each physical Escape
to an acknowledged prompt in the owned private window.

Mixed hidden-object bounds, general reference-frame orientation at clamped
borders, mouse/pixel parity, clipping-plane objects, named-plane icon/list input
and comparative performance remain unqualified. Newer AutoClose/PersistentClose
options and Edit history retain their existing compatibility limits. No full
native parity or continuous native curve-distance certificate is claimed.

```sh
cargo test -p viboceros-command --test surface_interpolation_plane_native
cargo test -p viboceros surface_interpolation_plane
python3 -m unittest tools.rhino_oracle.test_surface_interpolate_plane
```

References: [Rhino InterpCrvOnSrf](https://docs.mcneel.com/rhino/8/help/en-us/commands/interpcrvonsrf.htm),
[InfinitePlane](https://docs.mcneel.com/rhino/8/help/en-us/commands/iplane.htm).
The box projection and padding formula follows the open-source
`ON_PlaneSurface::CreatePseudoInfinitePlane` in
`third_party/opennurbs/opennurbs_planesurface.cpp`.
