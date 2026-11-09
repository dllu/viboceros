# Ordered ruled surfaces and loose ribbons

[Geometry guides](README.md) · [Validation record](../ribbon-ruled-validation.json)

`NurbsSurface::try_ruled_from_curves(a,b)` matches clamped degrees and normalized
knots, then interleaves homogeneous controls. U crosses between boundaries;
V follows their original traversal. Both domains are `[0,1]`. Closed and constant
curves are accepted. Independent weight gauges remain stored: they affect the
transverse clock, and opposite signs can produce poles. The constructor performs
no orientation sorting, seam movement or endpoint-weight normalization.

Structural callbacks and `viboceros_smlib::ruled_surface` share this policy with
usd-brep. Loose surface-offset walls now reuse it instead of a private copy.
The existing wall command regressions pass on both builds.

`try_loose_ribbon` offsets original Greville positions into polynomial curves,
builds ruled tensors, splits G1 breaks and constructs shared B-rep topology.
BothSides builds two sides and sews them along the original curve. The supplied
signed distance chooses the first side; output U spans `[0,abs(distance)]` and V
retains the source chart. The `_with_ruled` form delegates structural operations.
Grid and adaptive partitions now share closed boundary tips even when the
opposite boundary is open. Unrelated coincident diagonal tips remain distinct.

The local Python operation `loose_ribbon_geometry` accepts `curve`, signed
`distance`, optional `both_sides` and `normal` (default `[0,0,1]`). It returns a
complete `brep` definition, unchanged source readback and explicit false flags
for a constant-distance certificate and full Ribbon command parity.

Both builds certify complete surface-image agreement for 32 headless native
loose Ribbon cases at `1e-9`. Exact tensor bounds and full rectangular-domain
coverage handle differing face subdivisions; samples do not admit these results.
All support-surface 3DM round-trips pass. Vertex/edge geometry and face-loop edge
incidence match in 31 cases. The both-sided polycurve has four local faces versus
three native faces; the complete image agrees. Native face-reversal flags agree
in 30 cases, with a both-sided circle difference and the unresolved face mapping.
Loop winding and object order are not qualified by the incidence comparison.

The [Ribbon command](../commands/ribbon.md) now adds curve picking, distance
getters and object-history policies. [Polyline corners](polyline-ribbons.md) and
[ordinary smooth offsets](smooth-ribbons.md) have separate constructors.
Self-intersection trimming and native performance parity remain unfinished.
