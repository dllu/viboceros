# Higher-order BlendCrv qualification

[Command](commands/blend-curve.md) · [Machine-readable record](blend-curve-high-validation.json)

BlendCrv accepts independent G0–G4 constraints through the existing preview,
shape handles, sliding contacts, Flip and Trim/Join editor. The portable solver
and usd-brep backend share endpoint jets and minimum-degree interpolation.
A G3/G3 blend has degree seven; G4/G4 has degree nine.

The bounded qualification matches 25 command workflows in both builds and both
command/UI paths: six historical G3/G4 observations plus 19 new captures.
Sources include lines, an analytic arc, cubic/quartic/quintic polynomials and a
spatial rational cubic. Holdouts cover mixed orders, shape changes, contact
sliding, reversal, Flip and joining. Each build verifies 50 3DM round trips,
825 native point/tangent stations and 279 public derivative stations through
fourth order. Windowless egui pointer tests exercise G2, G3 and G4 shape edits.

The source jets are independently evaluated with one-sided homogeneous quotient
derivatives. A nonlinear source reparameterization test checks their normalized
third/fourth invariants. All 25 mixed endpoint-order pairs reproduce known
polynomials in both interpolation backends; existing Blend and G0–G2 cases remain
regression checks. Native output controls are expected data only.

Two measured Native policies matter:

- Accepted BlendCrv commands return `Nothing` with success=false. Admission also
  requires complete input, valid output, independent evaluations and history.
- Native G4 defaults constrain the blend's fourth derivative to zero even when
  the source has a nonzero fourth derivative. This is command compatibility;
  general mathematical G4 continuity is not claimed. The generic endpoint-jet
  API separately supports nonzero fourth targets.

Geometry retains a `1e-6` gate on coefficients and corresponding normalized-chart
samples. Native approximate arc-length domains have a separate `2e-6` parameter
error gate. For the historical G3 arc case, independent adaptive Simpson
quadrature measures length 10.8060956372845 versus Native domain span
10.8060968673357. Viboceros keeps the more accurate length. Derivatives are
compared on normalized charts with component error divided by
`max(1, abs(actual), abs(expected))`; the gate is `1e-6`, and absolute differences
are also recorded. Joined-domain differences can amplify fourth derivatives
without a comparable model-space discrepancy.

The earlier [G0–G2 record](blend-curve-validation.json) remains historical,
including its six then-unsupported G3/G4 diagnostics, three incomplete input
drivers and failed dispatch attempt. Promotion occurs only in the new record.

Reproduce checks and reconstruct the record:

```sh
python3 -m tools.rhino_oracle.check_blend_curve_high
python3 -m tools.rhino_oracle.report_blend_curve_high --implementation-revision <full-sha>
```

Captures run in private Xvfb; tests create no desktop windows.
[Point endpoints and partial joins](blend-curve-points.md) now have separate
qualification. Closed sources, SubCrv, symmetry/Alt edits, curvature display,
history re-editing, extreme higher-order scaling, continuous distance and
comparative speed remain unsupported or unqualified.
