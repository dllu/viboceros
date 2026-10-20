# InterpCrvOnSrf

Enter `InterpCrvOnSrf`, select a surface or B-rep face, and pick or type points.
A single preselected surface is accepted. Enter finishes; `Close` finishes a
closed draft and `Undo` removes the last point. Escape discards the draft and
preserves Redo. Points support relative coordinates and document units.

```text
InterpCrvOnSrf -4,-3,0 0,2,0 4,-2,0 Surface=<object-id>
InterpCrvOnSrf -3,-3,0 3,-2,0 2,3,0 -2,2,0 Close Surface=<brep-id> Face=0
```

`AutoClose=Yes|No` defaults to Yes. Returning to the first point after three
distinct locations makes a closed draft; Enter still commits it. The pointer
snaps to that location within 12 screen pixels. Hold Alt to suspend this
closure gesture. `PersistentClose=Yes|No` defaults to No and keeps subsequent
drafts closed. These preferences persist within the running app.

Points project to the selected trimmed face, including its boundary when a
point lies inside a hole. The interpolant uses the underlying surface and may
cross trims. Outputs use the current layer, have no name and remain unselected;
source geometry and selection are preserved. One document Undo removes the
output. Source, history, tolerance or selectability changes invalidate previews.

The kernel interpolates native UV coordinates with chord-spaced cubics; changing
the surface's UV scale can change the curve. It unwraps exact periodic chart
seams, preserves winding and splits the world-space image at seams. Closed UV
interpolation uses a periodic residual plus a linear winding term. Spatial
images use exact affine composition or certified cubic fitting with analytic
endpoint tangents. The fitting certificate bounds position, not curvature.

Every output passes a continuous surface-image bound within half the document
absolute tolerance and a separate interpolation-location check. If the UV
interpolant overshoots an open natural boundary, a private exact surface
extension supplies the support for this proof; the document source is unchanged.
Unsupported extensions, failed proofs or more than 256 input locations fail
before document edits. Closed interpolation requires three distinct locations.

Native SDK and command evidence, including periodic winding, rational patches,
trimmed point constraints and boundary overshoot, is recorded in
[the qualification report](../surface-interpolation-qualification.json).
The installed Rhino 8.32 build reports AutoClose and PersistentClose as unknown
options, although the current manual documents them. Their option behavior is
covered by local tests and is not marked as native-qualified. `Edit` history
and `InfinitePlane` point input are not implemented.

References: [Rhino command manual](https://docs.mcneel.com/rhino/8/help/en-us/commands/interpcrvonsrf.htm),
[public UV interpolation API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_InterpolatedCurveOnSurfaceUV_1.htm).
