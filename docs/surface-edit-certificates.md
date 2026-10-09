# Certified boundaries after surface edits

[Smooth](commands/smooth.md) · [Continuous correspondence](surface-curve-certificates.md)

`Brep::try_with_edited_single_surface` retains a single face's UV trim curves
and orientation when its control net changes. Rectangular boundaries keep exact
isocurves. Nonrectangular boundaries now require a continuous certificate in
addition to the existing bounded image proposal.

The proposal fitter retains its original 4,096-control ceiling and sampled
refinement policy. Its output is independently checked against the original UV
trim and edited surface, using exact rational extraction and outward interval or
exact Bernstein bounds. The proof covers the complete normalized-parameter
correspondence, including all original UV and spatial knot spans. The bound is
one quarter of document absolute tolerance; component tolerances are not enlarged.
Dense edited trims have an explicit 16-million-unit proof ceiling per image/use.
Immutable surface preparation is reused, while each proposal gets a fresh budget.
Degree, rational bit-size and subdivision limits remain the ordinary certificate
limits. The public certificate and pushup APIs keep their existing work budgets.
No sampled-only result is returned.
Shared edge reuse must certify the retained spatial edge against every UV use,
including reversed orientation. A constant fitted image can become a singular
trim only after a zero-error certificate proves exact collapse. The ordinary
B-rep validator remains responsible for topology. Certificates prove curve-image
correspondence, not global surface regularity, trim injectivity or shell embedding.

Unsupported weight signs/degrees, chart excursions, exhausted arithmetic or
fitting budgets return errors. Smooth stages the complete mixed edit before
replacing document objects, so such failures retain sources and history. Unchanged
surfaces keep the preceding no-op path.

Regressions cover diagonal boundaries and reversed faces, circular holes, shared
seams/poles, rejected mixed weights, explicit proof budgets and an excursion hidden
at old uniform fit stations. Document tests cover certified smoothing with groups,
attributes, geometry user text and independent Undo/Redo. Full application replay
uses the retained native Smooth capture; its native spatial comparisons remain
sampled nearest-locus witnesses. No fresh Rhino timing or capture is claimed.
See [source provenance](surface-edit-certificate-provenance.json).
