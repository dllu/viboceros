# STEP extrusion p-curve images

[Native STEP import](commands/import-step.md) · [Certified pushup](certified-surface-pushups.md)

Native linear-extrusion import now constructs spatial edges for diagonal and
curved UV p-curves, extending the preceding isocurve-only adapter. The face and
edge paths share tensor patch construction: U retains the converted directrix's
degree, controls, weights and knots; V is linear in the STEP extrusion parameter.
No display mesh substitutes for these editable surfaces or edges.

The new path requires a directrix whose conversion retains interior parameter
speed: lines, polylines, B-splines/NURBS, supported curve leaders, or non-conic
p-curves on planar/spline bases or supported linear extrusions. Conic and angular
directrices keep their preceding isocurve handling. Applying arbitrary UV curves
to their rationalized parameter speed would describe a different locus.

A sign-coherent UV control hull must stay inside the directrix's active domain.
Its axial control bounds define the edge's supporting V interval; constant axial
coordinates use an interval containing that value. The original p-curve domain
is retained. Certified pushup tries exact rational composition, then bounded
adaptive fitting if needed, and independently verifies the final complete image
at import tolerance. Certificate degree, weight, arithmetic and fitting limits
apply; inconclusive images fail native import before document insertion.

The existing globally affine line-extrusion route remains first, preserving its
unbounded parameter map. Source face conversion shares the same control-net
constructor, including old conic/isocurve cases. This extension changes edge
admission, without changing shell grouping, assembly or unit-conversion policy.

Direct tests cover polynomial/rational multi-span directrices, a parameter-curve
directrix, reversed/signed UV gauges, knot crossings, constant axial coordinates
and invalid charts. A serialized STEP triangle with a curved rational p-curve
imports both polynomial and rational extrusion faces, checks domains and every
oriented returned trim/edge certificate, and compares source-evaluation stations.
The preceding affine and isocurve regressions remain. This is local conversion
evidence; fresh Rhino cross-reader and performance comparisons remain unverified.
See [source provenance](step-extrusion-pcurve-provenance.json).
