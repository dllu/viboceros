# Section Refit development

`Refit=0.01` and `Simplify=Refit RefitTolerance=0.01` fit copies of real
profiles before point-end poles are added. Rebuild and Refit are mutually
exclusive, and authored rail stations are retained.

Compatibility remains incomplete at comparison epsilon `1e-7`:

| Cases | Constructed | Complete surface images proved |
| --- | ---: | ---: |
| Base SDK, command and controlled sweeps | 86/86 | 83/86 |
| Re-entered C1 profiles | 12/12 | 8/12 |
| Combined | 98/98 | 91/98 |

The [latest full replay](refit-separation-validation.json) covers all 98 cases on both
backends, retaining seven failing image assertions: three base fine profiles
and four fine C1 single profiles and pairs. Coarse profiles and disabled fitting
pass. The [earlier base record](refit-development-validation.json) retains its
measured source bindings for commit `46371ad4e`.

Exact projected distance bounds now establish a point on each failing surface
farther than `1e-7` from the complete native surface. These are geometric gaps.
The proof charges stored-coordinate and weight rounding; numerical closest
points only propose witnesses. A proved gap skips further correspondence trials.

UI regressions cover a circular section on a line: both tolerance getters,
`Simplify`, explicit and automatic stations, cancellation, and local and model
undo/redo. Broader UI qualification remains outstanding. Exact squared-distance
Bernstein bounds resolve the former concentric-circle placement failure.
Whole-surface comparisons charge rounding from U-degree elevation; matching
end rows alone cannot establish agreement.

Single smooth profiles use two independently certified fitting passes.
Straight and polygon profiles keep their separate path. The same profile can
produce different boundaries when supplied alone or in a pair. Batch fitting
checks original profiles and intermediate curves, reconstructs at shared
interpolation stations, and requires a fresh complete original-source bound.
Inserting knots after independent reconstruction gives different geometry.
Target-knot witnesses and tangent frames now propose correspondences for the
final reconstruction certificate, removing two false construction failures.
The complete image proof still gates acceptance; fitting and subdivision are
unchanged. Regression tests cover independent parameter clocks, displaced
curves and retraced source visits.

Known span correspondences receive progressively refined proposal budgets,
scaled to their active spline spans and capped at 1024 intervals. Distance and
arithmetic-work limits remain unchanged. A nominal Refit tolerance is not a
final source-distance guarantee: two passes accumulate error, and the
[exact regression](../../tools/rhino_oracle/test_sweep1_refit.py) retains a native
0.001 fit more than 0.0015 from a source point.

The new boundary-construction test uses Rhino-observed subdivisions only in
test inputs. Independently constructed G1 pieces match all four complete native
boundary curves, including the fine quadratic and circle, within `1e-7`.
Production fitting never reads those cuts or native control nets. The remaining
fine single-profile problem is subdivision selection.

Public API diagnostics retain controlled handles, `Curve.Fit`, legacy Sweep,
SDK sweep tolerance, closest points and equivalent rational storage forms.
The legacy and modern Sweep APIs agree on their tested inputs; varying SDK
sweep tolerance leaves the five tested profile definitions unchanged.
All 140 public closest-point queries match Viboceros within `1e-7`. C1 re-entry
uses independently constructed input curves, not purported Rhino intermediates.
[Representation diagnostics](refit-representation-validation.json) certify 12
equivalent inputs at `1e-12`, then compare 16 native sweep images. Knot insertion
changes two images beyond `1e-7`; degree elevation and affine domains retain
the tested images. The [clock correction](refit-clock-validation.json) resolves
all 16 fresh multi-span batches and adds 450 precision queries. Numerical NURBS
batches share request normalization; fitting retains its measured public request
of `0.01`, giving an effective tolerance of `1e-4`.
[Public fitting diagnostics](refit-cubic-validation.json) reproduce 480 native
continuity queries and certify 96 full or trimmed input curves within `1.2e-14`.
Public `MakeCompatible` results exactly reproduce 32 complete Sweep boundary
definitions; its zero and one-degree angle controls retain 48 complete curve
definitions. These cases isolate the remaining fitting differences from Sweep
construction. Public cubic-Bezier partitions are recorded separately and do not
establish the private subdivision rule. Production fitting is unchanged.
The other historical qualification inventories still describe `main` before
this branch and must be refreshed before merging.
