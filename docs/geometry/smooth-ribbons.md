# Ordinary smooth ribbons

[Geometry guides](README.md) · [Ruled image bounds](ruled-images.md)

`try_smooth_ribbon` builds planar regular NURBS ribbons. A sharp offset of the
control polygon supplies a candidate with the original knots and weights.
Endpoint and interior checks compare it with the true normal-offset locus.
Accepted candidates retain rational gauges and clocks, including quadratic
circles, projective arcs and nearly linear curves. Otherwise adaptive cubic
Hermite segments interpolate offset positions and tangents, using half the
model tolerance for their interior fit checks. Ordinary Offset retains its
existing quarter-tolerance policy and native fitting producer.

The constructor builds ordered ruled tensors, partitions G1 breaks and sews
BothSides along the original curve. U spans `[0,abs(distance)]`; V retains the
original source chart. Independent source and offset gauges remain stored.
Polynomial fitted offsets have unit weights. The `_with_ruled` form delegates
structural basis edits to `viboceros_smlib::ruled_surface`.
Sewing can use the original source as an intermediate certificate: both stored
boundary curves must pass complete bounds against it. This avoids expensive
products of two densely refined copies without replacing either curve.

`smooth_ribbon_geometry` is a readonly local Python operation accepting `curve`,
signed `distance`, optional `both_sides` and `normal` (default Z). It returns a
complete `brep`, unchanged source readbacks and false flags for full Ribbon
command parity and a constant-distance certificate. `loose_ribbon_geometry`
retains its previous behavior through the same topology helper.

Fit checks are sampled; they do not certify offset distance between stations.
Native comparisons use complete image bounds and full trim-domain coverage,
plus complete edge/vertex geometry and face-loop incidence checks. Boundary
witnesses are themselves certified and never replace the actual stored curves.
U correspondence, winding, object policy and native performance parity remain
unqualified. Kinks, cusps and self-intersection trimming require further command
work. `Ribbon` remains unregistered.

[Frozen validation](../ribbon-smooth-validation.json) replays 60 native cases on
both backends with strict `1e-9` complete-image and incidence gates. Reproduce
the evidence audit with `python3 -m tools.rhino_oracle.report_ribbon_smooth`.
