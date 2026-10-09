# Section

`Section start-point end-point` intersects selected curves, surfaces, B-reps and
meshes with a plane perpendicular to the current construction plane. The picked
line defines the plane's trace; it is projected into the CPlane. Coincident or
CPlane-normal-only picks are rejected. Standalone points and point clouds are
excluded from source selection, matching the recorded native command.

Bare `Section` uses object selection and the shared two-point drafting getter.
After creating a section, pick another plane or press Enter to finish. Esc
cancels the pending plane. Inputs stay in the model. Outputs and an optional
output group form one Undo step per accepted plane.

Options are `ExtendSection=Yes|No`,
`AssignProperties=ByCurrentLayer|ByInputObject`, `Output=All|CurvesOnly`, and
`GroupObjectsBySectionPlane=Yes|No`. ExtendSection=No limits output to the picked
span; the plane extends along CPlane Z. ByInputObject copies source attributes;
ByCurrentLayer uses fresh current-layer attributes. Grouping collects all output
from the plane. Section styles/hatches are not implemented, so All currently
uses the same curve/point output as CurvesOnly.

The surface/B-rep path reuses the validated intersection kernel and stages every
output before admission. Its existing surface-family limits still apply;
unsupported intersections fail without partial output. Coplanar planar surface
sections use the natural border policy. Mesh sections use exact dyadic plane
and edge arithmetic, shared topology endpoints, winding-aware path direction and
branch-aware tracing. Finite mesh output is clipped to the picked span.

Thirteen of fourteen native command recipes match strict geometry/domain,
metadata and grouping checks. The remaining coplanar mesh recipe has the same
rectangular boundary but a different seam and two extra collinear vertices in
Rhino; its parameter-domain mismatch is retained as a diagnostic. The raw replay
report is deliberately not a complete compatibility pass. See
[comparison](../section-command-comparison.json),
[coplanar mesh diagnostic](../section-coplanar-mesh-diagnostic.json) and
[provenance](../section-command-provenance.json).

Broader B-rep/curved input qualification, nonmanifold mesh junction policy,
closed-mesh seam fidelity, section styles/hatches, remembered options and
performance remain open. Full Rhino compatibility is still in progress.

Reference: [Section and Contour](https://docs.mcneel.com/rhino/8/help/en-us/commands/contour.htm#Section).
