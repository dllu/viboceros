# Section Refit development

`Refit=0.01` and `Simplify=Refit RefitTolerance=0.01` fit copies of real
profiles before point-end poles are added. Rebuild and Refit are mutually
exclusive, and authored rail stations are retained.

Compatibility remains incomplete at comparison epsilon `1e-7`:

| Cases | Constructed | Complete surface images proved |
| --- | ---: | ---: |
| Base SDK, command and controlled sweeps | 86/86 | 83/86 |
| Re-entered C1 profiles | 10/12 | 8/12 |
| Combined | 96/98 | 91/98 |

The [base replay](refit-development-validation.json) retains its measured source
bindings for commit `46371ad4e`. The
[re-entry record](refit-reentry-validation.json) adds all 12 C1 cases, including
the failing assertions. The three base fine-profile images still differ. Two
fine C1 single-profile images also differ, and two fine C1 profile pairs fail
to construct. Coarse single profiles, coarse pairs and disabled fitting pass
the new complete-image comparisons on both backends.

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
The other historical qualification inventories still describe `main` before
this branch and must be refreshed before merging.
