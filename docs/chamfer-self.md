# Chamfer on one parent

[Command syntax](commands/chamfer.md) · [Validation record](chamfer-self-validation.json)

Pick two locations on a polyline, polycurve or kinked NURBS to select neighboring
smooth pieces. Chamfer measures each Euclidean setback from their junction.
Reversing the picks swaps the distances. Flat subdivisions inside the original
interval form one picked piece. Setbacks can cross flat or kinked earlier spans;
they search the full retained prefix/suffix, rather than stopping at a leaf end.
Closed parents search in both directions from the junction with a temporary seam.
The selected junction determines the retained cyclic interval; an asymmetric
straight bevel's tangent does not reliably identify that interval.

Trim/Join preserve the parent's ID, attributes and groups. Without Join, the
first open remainder retains that identity and the second receives the parent's
attributes with fresh identity and no groups. The bevel is unnamed, ungrouped
geometry on the current layer. Trim=No adds only that bevel. Results are
deselected. Open zero-distance cuts still split the parent; Join produces a
polycurve through its unchanged corner. Closed zero-distance cuts fail. The
original seam remains a native picking boundary even when geometrically smooth.

Headless calls accept repeated explicit source IDs, or one selected parent with
both Pick1 and Pick2. Native NURBS extraction uses the same usd-brep operations as
Fillet. Smooth-piece decomposition and immutable result assembly are shared.
Source interval and candidate scans are linear in the number of pieces.

Four immutable licensed-Rhino captures contain 46 complete workflows: 35 edits,
ten failures and one cancellation, with 91 acknowledged physical picks and 3,861
public point/tangent stations. Both builds replay 35 successful commands through
prepared, selected-parent and explicit-source paths, plus all 46 UI workflows.
Checks compare coefficients, knots, original parameter samples, tangents, IDs,
attributes, groups, selection, Undo/Redo and 82 3DM round trips per build at 1e-6
(including the repeated-mouse-pick test).
Comparisons use the original parameters directly: affine remapping can move a
sample across a nonsmooth knot by one floating-point step and select its other
tangent. The ten API failures also verify full document immutability and Redo.

```sh
python3 -m tools.rhino_oracle.check_chamfer_self
```

Captures use private Xvfb displays; application tests create no windows. Distinct
closed-source pairs remain unsupported. This finite corpus does not qualify all
folded, self-intersecting or oscillatory branches, a continuous distance bound,
pixel equality or comparative performance. Shared Fillet/Chamfer regressions,
both workspace Clippy configurations, formatting, Python protocol/evidence tests
and affected SubD source qualification checks run with this suite.
