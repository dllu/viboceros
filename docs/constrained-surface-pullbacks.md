# Pullbacks with fixed UV endpoints

[Continuous certificates](surface-curve-certificates.md) · [Surface splitting](commands/split.md)

`NurbsSurface::try_pullback_curve_certified_with_endpoints(spatial, [start, end], tolerance)`
constructs a UV curve with the two caller-supplied endpoints. Its complete surface
image must follow the original spatial curve within absolute tolerance, with
both parameter domains mapped affinely to `[0,1]`. The spatial curve, surface,
and source domain remain unchanged. Endpoints must stay in the natural surface
domain and within model-space tolerance of the respective spatial endpoints.

An eligible exact inverse retains its NURBS structure when adjusting its clamped
end controls receives a certificate. A generic straight UV proposal handles
isocurves whose spatial endpoints coincide at a periodic seam but whose UV
endpoints are distinct. Every such proposal still needs a continuous proof.
Other regular curves use cubic Hermite fitting with the requested endpoints in
the endpoint nodes. Derivatives are projected through the surface Jacobian at
those points. Inconclusive spans subdivide; every accepted span and the assembled
curve are certified against the original exact spatial spline.

Surface cutting uses the fixed-endpoint fitter when adjusting an existing trim
or retaining a bilinear proposal cannot certify. It incorporates the shared
topological endpoints while refining the fit. The cutting-pullback controller
now lives in its own `brep/surface_cut_pullback` module.

## Regression evidence

A non-affine planar graph has a cubic pullback whose maximum error is just
inside tolerance. The requested end moves only about a quarter of the allowed
model distance. Moving the last control of the old fit pushes an interior point
outside tolerance. Fixed-endpoint fitting refines the curve and preserves the
shared endpoint with a complete certificate.

Closed cylindrical, spherical-equator, and toroidal isocurves exercise distinct
seam endpoints in both traversal directions. Further tests retain unclamped
spatial input and its domain, and reject outside-domain or distant endpoints
even when the caller's relative tolerance is large.

The [eight public SDK recipes](../tools/rhino_oracle/fixtures/surface_pullback_endpoints.json)
and [raw records](../tools/rhino_oracle/observations/surface_pullback_endpoints.json)
were captured on private Xvfb in a fresh settings scheme. They retain full
source and native pullback control definitions, source purity checks, 129 paired
sample stations, closest-locus errors, and independent endpoint image witnesses.
See [provenance](constrained-surface-pullback-provenance.json).

Rhino's public [`Surface.Pullback`](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_Pullback.htm)
defines a geometric fit to the surface points closest to the spatial curve; it
does not accept fixed UV endpoints or promise normalized-parameter agreement.
The two planar records have matched-parameter sample errors about `0.086`, while
their closest-locus samples remain below `0.00108`. Those native parameter speed
differences remain explicit. The six closed isocurves use public
[`Surface.IsoCurve(int, double)`](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_IsoCurve_2.htm).
Native torus parameter domains differ from the local primitive constructor;
replay uses the retained complete definitions and domains.

The Rust protocol tests replay all eight retained source definitions, compare
their 1,032 spatial stations and native surface-image stations at `1e-11`, check
the independent endpoint witnesses, and require certified local fits with the
exact requested endpoints. They also exercise omitted endpoints, malformed
constraints, invalid limits, and endpoints outside the domain or model-space
budget. Python tests verify the closed recipes, launch isolation, raw record
coverage, and capture hashes.

Finite native samples do not prove continuous errors. The local fixed-endpoint
contract is checked with exact rational certificates. These queries do not prove
trim simplicity, arbitrary periodic chart tracking, singular pullback fitting,
surface injectivity, or general curved Boolean construction. Certificate work,
degree, rational-size, and subdivision limits remain in effect.

## Python debugging API

The local `surface_pullback_certified` operation accepts a full `surface`
definition, a `spatial_curve` definition, positive `limit`, and optional
`endpoints: [[u0,v0],[u1,v1]]`. The request's relative and angular tolerances are
used for numerical proposals; the proof always uses the operation's absolute
limit. Results retain the full UV NURBS definition with zero-Z wire controls,
`bound`, `certified`, `fixed_endpoints`, and `normalized_curve_domains`
correspondence. Omitted endpoints use the ordinary certified fitter.
The [runnable local request](../tools/rhino_oracle/fixtures/surface_pullback_certified.json)
retains the full inputs and constraints from all eight native source records.
An October 6, 2026 Python CLI run returned seven controls and a continuous bound
of `0.0007414553029511127` for each planar direction, below its input limit
`0.0028772692054040688`. All six closed isocurve inputs returned two UV controls
and an exact zero bound against their retained spatial definitions. These are
local fitting and certificate results, separate from native pullback parity.

```sh
cargo test --release -p viboceros-geometry fixed_
cargo test --release -p viboceros-geometry valid_shared_endpoint_adjustment
cargo test --release -p viboceros-oracle surface_curve_image
python3 -m unittest tools.rhino_oracle.test_surface_pullback_endpoints
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_pullback_certified.json --timeout 600 --output /tmp/pullback-endpoints-local.json

# Use a fresh private scheme for every capture.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/surface_pullback_endpoints.json --scheme VibocerosOraclePullbackEndpointsExample --timeout 600 --output /tmp/pullback-endpoints-rhino.json
```
