# Arc Extension

Start `Arc`, choose `Extension`, then pick an open curve or surface edge near
the end to continue. Pick an endpoint to create a tangent arc as a separate
object. The source geometry, attributes and parameterization remain intact.
The nearer geometric endpoint determines which end is used, including for
nonuniform NURBS parameterizations.

Choose `Center` after the source pick to specify the circle's center. The
center is projected onto the plane perpendicular to the source's endpoint
tangent. Then pick an endpoint or enter a signed angle in degrees. Cursor
motion chooses the sweep when picking an endpoint, using the same state as
Arc Tilted. Negative numeric angles reverse the sweep. The viewport previews
use the same arc construction and output conversion as completion.

`Deformable` can be chosen before Extension to produce editable NURBS output
with Arc's degree and point-count settings. Successful construction saves
these settings; cancellation discards pending changes. Source picks are
temporary selections: completion and cancellation release them. Geometry
changes, deletion, locking, hiding or tolerance changes discard a stale
source and return to the curve picker.

The command API names its source explicitly and supplies the pick location:

```text
Arc Extension Curve=id 9.5,0,0 13,4,0
Arc Extension Curve=id 9.5,0,0 Center 12,4,0 90
Arc Extension Edge=id,0 9.5,0,0 13,-4,0
Arc Deformable Degree=5 PointCount=8 Extension Curve=id 9.5,0,0 13,4,0
```

Replace `id` with a document object ID. Ordinary points, relative coordinates,
snaps, filters and construction-plane coordinates use the common input path.
Picking the source sets the relative input base to the chosen curve endpoint.
Explicit `Direction=Clockwise|Counterclockwise` before a Center endpoint
provides a deterministic command-API sweep. Interactive picking derives it
from cursor motion.

Completion owns one `Arc` undo/redo entry. Previewing, rejecting input and
cancellation create no helper objects or geometry history. The implementation
reuses immutable curve references, the existing start/tangent/end constructor,
normal-plane projection and the shared Arc sweep getter.

[Validation](../arc-extension-validation.json) records full native NURBS,
selection and undo/redo comparisons at absolute tolerance `1e-6` on both
backends. Native typed Center endpoints did not complete in the diagnostic
capture; endpoint qualification uses physical clicks and independent viewport
ray receipts, including the initial cursor. In the captured Rhino 8 build,
`Length` at the Extension Center endpoint starts the global length-measurement
command. This implementation offers endpoints and angles there. Collinear
default endpoints are rejected; exact native error/termination behavior,
all cursor trajectories and performance parity remain unqualified.

Reference: McNeel's public [Arc command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/arc.htm).
