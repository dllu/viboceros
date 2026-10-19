# Automatic surface domains

`Reparameterize Automatic` now sets each surface parameter range from sampled
isocurve lengths. For each direction it extracts eleven isocurves at transverse
fractions `0, 0.1, …, 1`, excludes collapsed boundaries and zero-length curves,
and averages the positive lengths. Boundary collapse uses the existing
OpenNURBS coordinate-coincidence convention on clamped control rows, keeping
roundoff-sized pole curves out of the mean after translation. This inference
comes from public Rhino command observations, including
warped patches that distinguish boundary-inclusive sampling from nine interior
samples. Spherical pole isocurves collapse, so their average uses nine curves.

Each length uses the existing Cash–Karp 5/4 quadrature with fractional tolerance
`1e-8`, matching the public default
[Curve.GetLength](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_GetLength.htm)
query. This reproduces the small numerical length bias in the recorded command
domains. The previous maximum control-polygon policy gave `16` for a radius-two
cylinder; its recorded command U range ends at `12.566370643255096`. The NURBS
sphere's recorded U range ends at `8.781407175018794`.

`ArcLengthSampler::quadrature_length_estimate` shares the existing trace
integrator and its bounded step budget. Variable-speed rational line spans opt
into quadrature for this estimate. The existing `total_length`, precise distance
queries and integration checkpoint policy retain their behavior. Degree-one
NURBS curves use private unit parameter ranges during numerical integration,
including sources with large parameter origins. The average scales lengths by
their maximum before compensated summation, avoiding overflow in the sum.
Zero-width or zero-height results are rejected before any document edit.

The original 30 face-command recipes retain strict domain checks alongside
surface and lifted-trim image checks, topology, metadata, history and 3dm
readback. The two earlier curved Automatic domain failures are no longer
accepted as expected mismatches. Twenty new independent surface definitions
add bilinear warps, rational bilinear patches, weighted quadratic patches,
unclamped knots, cylinders and cones, a sphere with transposed or changed
charts, spatial scales, translation, a common weight gauge, rational changes of
parameter, degree elevation and knot insertion. Four further captures exercise
`1e-12` and `1e-5` document tolerances. Actual command domains are checked at the
caller epsilon capped at `1e-8`, with geometry checked at the model epsilon.

The first unclamped capture exposed automatic crease splitting during generic
Rhino object insertion. Its complete failed command observation is retained.
The final source recipes use the public insertion overload that preserves the
supplied B-rep face count, allowing both engines to edit the same topology.
The raw source files, command histories, API controls and tolerance variants
remain unnormalized in the retained observations.

These checks qualify the recorded inputs and document policies. They do not
prove continuous global Rhino equality, arbitrary conditioning or universal
performance bounds. Numerical estimates remain separate from geometric
acceptance tolerances. The source geometry, neighboring faces and explicit
parameter ranges retain the existing command's contracts.

[Validation](automatic-surface-domains-validation.json) records the executed
checks, observations, source versions and the prior-policy comparison.
