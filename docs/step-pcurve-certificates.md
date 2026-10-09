# STEP spline p-curve qualification

[Native STEP import](commands/import-step.md) · [Continuous certificates](surface-curve-certificates.md)

Spline UV p-curve adapters construct spatial NURBS edges through affine mapping,
isocurve extraction, rational reparameterization or Bernstein composition.
Subdivision, crossing isolation, rounded parameter restoration, endpoint alignment
and span joining remain proposal construction. The final edge receives an independent continuous
correspondence certificate against the original stored UV curve and original
spline surface at the caller's absolute import tolerance.

The proof retains the original UV and spatial knot spans through exact rational
restriction. Rounded split curves and snapped endpoints never replace that
reference. A successful result bounds every corresponding point, rather than
only sampled stations. Unsupported or inconclusive proofs fail the native import;
there is no automatic mesh fallback. The shared qualification layer checks every
`PCURVE` edge whose basis is a B-spline or NURBS surface, including straight and
rational isocurves and affine spline patches. Analytic angular adapters, planes,
and explicit 3D-edge adapters retain their preceding contracts.

Full-order interior surface knots separate independent control nets. If the
complete sign-coherent UV control hull lies inside one component, the certificate
reference is selected by copying original controls and knot slices. No insertion,
clamping arithmetic or fitting changes that reference. A path exactly on a break
selects the following component, matching right-sided evaluation. A hull that
crosses or approaches a discontinuity from its preceding side is rejected rather
than treating the source as continuous.

The import tolerance follows nested surface/intersection-curve leaders and
parameter-curve sweep directrices. Degree, rational bit-size, work and subdivision
limits are those of the ordinary surface-curve certificate. Shell conversion
completes before document insertion, retaining atomic import failure.

Tests reject an edge excursion invisible at endpoints and midpoint, check exact
component slices in both parameter directions and retain the knot-line convention.
The existing curved/rational crossing matrix covers tangencies, simultaneous
crossings, irrational parameters and repeated crossings. Serialized STEP import
also independently certifies every oriented returned trim/edge on its native face.
An additional rational straight-path regression succeeds at model tolerance and
rejects the same rounded proposal at a stricter tolerance, checking that the caller's
actual precision requirement reaches the qualifier.
These establish local conversion correctness for those cases. Fresh Rhino
cross-reader or performance evidence is not claimed.
See [current source provenance](step-spline-pcurve-provenance.json) and the
[preceding curved-only snapshot](step-pcurve-certificate-provenance.json).
