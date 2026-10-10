# Curved Divide and exact chord roots

The [55 curved recipes](../tools/rhino_oracle/fixtures/divide_curved.json) cover
count, length and chord division on an arc, ellipse, quadratic NURBS, rational
3D cubic NURBS and line/arc/line polycurve. Each checks point endpoints,
Split/remainder choices, domains, seventeen curve samples, source retention,
attributes and output groups in creation order. Licensed Rhino 8 observations
were captured under private Xvfb and a private settings scheme.

Twenty chord recipes match at 1e-9 absolute plus 1e-12 relative epsilon. The
35 count/length recipes retain approximately 1e-7 differences in native
numerical stations and split domains; they are qualified at 5e-7 absolute plus
1e-12 relative epsilon. The tight report retains every difference. These
recipes do not prove arbitrary rational-curve or performance parity.

The [four extreme recipes](../tools/rhino_oracle/fixtures/divide_chord_extreme.json)
exposed two defects. A rational line with weight ratio 2^100 needs a station
before 2^-64 of its domain; the old depth cutoff skipped it and returned two
points instead of three. A non-dyadic turnaround was rejected because chord
division required an arc-length integral that did not converge.

The root search now uses an explicit ordered stack and exact rational bounds,
with a subdivision-work budget instead of a fixed-depth answer. At parameter
resolution, one Bernstein sign variation certifies a real root; ambiguous
multiple variations use an exact bounded-degree Sturm sequence. A near-contact
complex pair cannot become a fabricated station. Denominator validation is
separate from arc length and checks every span before admitting any station.
Pole, even-multiplicity contact, extreme weight and root-free near-contact
regressions pass. Unrepresentable forward parameters or exhausted work still
return errors rather than partial document edits.

The two extreme point recipes match with their documented epsilons. Both Split
recipes remain diagnostics: native Rhino emits one unsplit weighted line, and
its turnaround cut uses parameter 0.4000499550246236 instead of the exact
contact at 0.4. Local geometry follows the independent exact stations. Raw
native/local outputs and full field comparisons are retained, without fitting
the implementation to these discrepancies. Their output count/parameter
differences are not qualified by the point comparison. Cusp arc-length
integration, general extreme domains, native Split boundary policies and
broader geometry/performance qualification remain open.

SDK source inspection also corrected an earlier harness assumption: Circle and
Arc recipes produce Rhino ArcCurve objects, while Ellipse produces NurbsCurve.
The local oracle no longer promotes the Circle recipe to NURBS. Four additional
[explicit NURBS-circle recipes](../tools/rhino_oracle/fixtures/divide_nurbs_circle.json)
show that native Split retains rational domains and sampled parameterization.
The whole-circle analytic carrier used previously was incorrect for these
inputs, with sampled differences up to 0.0796; Split now trims the original
NURBS at its original parameters. The native source records, rejected output
and final full-field comparisons are retained.

The final NURBS-circle replay matches all four recipes at 5e-7 absolute plus
1e-12 relative epsilon, with the chord Split case also matching at 1e-9. Its
maximum field difference is 1.1861457949180476e-7. The corrected analytic-circle
source replay keeps the preceding 27 tight/30 qualified matches. Reports are
[curved](divide-curved-comparison.json), [extreme chords](divide-chord-extreme-comparison.json),
[NURBS circles](divide-nurbs-circle-comparison.json) and
[corrected circle sources](divide-circle-source-comparison.json).

The optional native-only `inspect_sources` probe flag records the SDK runtime
types before Divide. Its source-type diagnostic is kept separately from geometry
replay; it is not a cross-engine comparison fixture.

[Provenance](divide-curved-provenance.json) records exact sources and final
validation. Detailed reports retain the preceding rejected implementation and
the final comparisons.
