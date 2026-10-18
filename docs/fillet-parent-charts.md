# Fillet parent parameter charts

`fillet_surfaces_with_parents` now returns changed parents in their original
U/V coordinate ranges. The rolling-ball producer still works on normalized
private inputs. After export, Rust affinely maps each parent surface and its
p-curves together, retaining the spatial vertices, edges, tolerances, incidence,
orientation, trim classification, rational weights and curve parameter knots.
Unchanged charts skip the mapping. The fillet sheet keeps its generated chart.

The shared `Brep::try_reparameterized_face_uv` operation accepts face indices and
new increasing U/V ranges. It stages the change, scales UV tolerance metadata,
and runs ordinary B-rep validation before returning. P-curve controls can lie
outside the surface's rectangle, so the mapper does not clamp them. It anchors
near the closer endpoint and reuses the existing scaled-ratio arithmetic.
Charts that lose too much coefficient precision are declined; model and
component spatial tolerances are never enlarged to accept them.

FilletSrf also restores the full underlying support of an existing rectangular
source face after using its clipped support for construction. The result keeps
the original coordinate ranges and trim location, including reversed source
faces. Clip edges inside the full support become interior isocurves; edges on
its boundary retain their side labels. Joining uses the existing native
stitcher and UV-axis restoration.
Preview and acceptance therefore expose the same parent coordinates, and
undo/redo retain the original objects and charts.

Thirty-two fresh RhinoCommon parent-result recipes exercise trim and split in
four plane quadrants, cylinder picks, transposed UV axes and a rational change
of parameter. Each runs with ordinary affine ranges and strongly unequal U/V
ranges. Complete raw SDK definitions, topology, areas and queries are retained
in [the reference](../tools/rhino_oracle/observations/surface_fillet_charts.json),
with [replay inputs](../tools/rhino_oracle/fixtures/surface_fillet_charts.json).
All 32 retain source purity, result counts and the exact original domain
endpoints. Twenty-four also pass the same-request SDK geometry checks at the
existing `1e-8` model epsilon. Eight strongly anisotropic curved cases expose
an SDK conditioning defect: the returned fillets have seven longitudinal
controls, sampled radius errors above `1e-4`, and parent edge tolerances far
above the requested tolerance. Repeating those eight with SDK tolerances of
`1e-10`, `1e-12` and `1e-14` returns identical complete values. These literal
SDK geometry mismatches remain explicit negative controls.

For the geometric invariance checks, the inputs normalize to exactly the same
NURBS coefficients as their earlier canonical-UV recipes and retain the same
physical picks. The native results are compared with those accurate canonical
SDK regions at `1e-8`, with separate analytic radius witnesses. The fresh
unconditioned observations still supply every original domain endpoint; their
raw definitions, inflated component tolerances, query answers and geometry
differences remain unchanged in the audit. This establishes chart restoration
and canonical geometric agreement; it does not establish literal geometry
parity for the eight SDK conditioning failures. No comparison or construction
tolerance is widened.

The rectangular region helper uses its existing `1e-12` coefficient filter in
normalized UV units so it qualifies the same regions under a chart change.
Raw coefficients and full protocol differences remain unnormalized.

Independent regressions cover rational curved holes, periodic seams, singular
pole trims, reversed faces, shared edges, large chart origins, unequal ranges,
invalid requests and insufficient coordinate precision. Native parent tests
also retain the existing extreme spatial scales and signed-weight gauges.
These checks do not establish continuous global Rhino equality, general input
conditioning, or general performance bounds. Curved Extend=Yes, arbitrary
trimmed fillet inputs and the existing closed-cylinder certificate limit remain
outside this change.

[Validation](fillet-parent-charts-validation.json) records the executed checks,
before/after protocol audits, reference capture and source versions.
