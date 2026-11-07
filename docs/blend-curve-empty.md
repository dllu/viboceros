# BlendCrv endpoint trimming and joining

[Command reference](commands/blend-curve.md) · [Validation record](blend-curve-empty-validation.json)

When Trim would retain an exactly zero-length source interval, Rhino keeps the
full source. Viboceros follows that behavior for whole curves and private SubCrv
portions. A nonzero interval still trims normally. This removes the previous
`InvalidCurveTrimInterval` failure after flipping a natural endpoint.

Join orients a whole untrimmed source using its connected endpoint, independently
of the blend tangent's Flip setting. This also fixes flipped sources being left
outside the joined result when Trim is disabled. Disconnected sources remain
unchanged. Existing ownership rules apply: the first participating whole source
keeps its identity, attributes and groups; private portions leave parents intact.

Two immutable Native captures contain 27 complete workflows: 26 acceptances and
one cancellation retaining pending redo. They cover either/both endpoint flips,
lines, arcs, cubic curves, reversed sources, G0/G2/G4, point endpoints, temporary
portions, joins with Trim disabled and short nonzero trims. The corpus includes
77 physical picks, 858 independent point/tangent stations and 300 derivative
stations. Native reports `Nothing` with success=false after accepted blends;
the document changes establish acceptance.

Command and windowless UI replays run in portable and native-smlib builds. They
check NURBS coefficients, samples, normalized derivatives, attributes, source
identity, selection, undo/redo and 3DM round trips. Geometry and normalized
derivative gates are 1e-6; parameter domains use the existing 2e-6 absolute floor
and 1e-8 relative length tolerance. Maximum geometry and normalized derivative
differences are 3.92e-8 and 1.62e-8, respectively.
Original failing test logs remain in the proof
directory, including a premature run before the capture was published. A broader
portable library run had 1,676 passes and one stale test expecting Line admission
to depend on document tolerance. That test now exercises Polyline, which uses
that policy; the focused tolerance suite is rerun in both builds. Line's existing
Native-qualified admission behavior is unchanged.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_empty
```

Captures use private Xvfb displays and do not open desktop windows. Surface-edge
trimming, arbitrary self-blends, continuous distance and comparative performance
remain unqualified. Earlier higher-continuity and approximate-length diagnostics,
curvature display and history reediting remain unchanged.
