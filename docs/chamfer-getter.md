# Chamfer getters

[Command syntax](commands/chamfer.md) · [Validation record](chamfer-getter-validation.json)

Chamfer now has ordered viewport source picks, sequential distance getters,
shared-anchor measurements, unit/arithmetic input and remembered options.
Preparation constructs geometry without editing the document. A shared curve
edit plan admits Chamfer and Fillet results with source identity, attributes,
groups, selection and history handled consistently. Stale sources invalidate a
pending session or prepared result.

Setbacks use endpoint sphere crossings rather than arc-length integration.
Lines and arcs have analytic solutions. NURBS use exact dyadic Bernstein sphere
equations to find the first crossing in the original parameter chart. One query
finds one setback without generating all equal-chord stations. Existing
portable/usd-brep operations trim or extend retained pieces.

Three immutable licensed-Rhino captures contain 31 completed workflows: 21
successes and ten cancellations. Both builds replay 20 supported successes
through prepared and explicit-source APIs and 30 workflows through the UI.
Checks compare coefficients, knots, native parameter samples, tangents,
attributes, groups, IDs, selection, Undo/Redo and 50 3DM round trips per build at
1e-6. Cases cover defaults, partial-distance cancellation, measurements,
negative-value correction, units, arithmetic, reversed order, zero distances,
arc and tangent extensions, NURBS contact and tilted lines. Synthetic viewport
mouse events exercise source selection and measured distance input.

The remaining native success uses distance 9 on a length-4 line: Rhino retains a
line ending at x=13 but starts its bevel at x=-5. Viboceros rejects that case
without editing the document or consuming Redo. Distinct closed-source pairs and all possible intersection branches remain
unsupported or unqualified. [Single-parent Chamfer](chamfer-self.md) has a
separate qualification. There is
no continuous-distance, pixel-equality or comparative performance claim.

Two failed cancellation drivers published no observations. Chamfer ignored
both typed `_Cancel` and in-process SendKeys Escape. The successful driver sends
physical Escape only to the verified owned private window. The pilot predates
external key diagnostics; its frozen driver, sent token and public Cancel event
remain available. Later captures additionally record physical key receipts.
The discovery measurement recipe includes a redundant anchor: Native accepts
zero as the second distance and ignores the following coordinate during source
selection. The corrected shared-anchor workflow is a separate holdout.

```sh
python3 -m tools.rhino_oracle.check_chamfer_getter
```

Captures use private Xvfb displays. Application tests create no desktop windows.
The runner also checks Fillet regressions, both workspace Clippy configurations,
formatting, Python protocol tests and affected SubD source qualification hashes.
