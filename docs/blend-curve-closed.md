# BlendCrv closed sources

[Command guide](commands/blend-curve.md) · [Validation record](blend-curve-closed-validation.json)

Whole closed curves use the picked contact in both BlendStart modes. At the
first end the initial direction decreases the source parameter; the second end
uses the opposite orientation. Open inputs retain their existing CurveEnd and
PickPoint behavior. Contacts can move through a seam, and Flip can distinguish
two references to the same curve by the nearest captured contact.

Closed parents supply constraints without being trimmed or consumed. Eligible
open inputs can still trim/join and contribute their identity and attributes.
Closed-only blends remain new NURBS curves. Interactive Join needs an eligible
open input and distinct whole sources. Trim needs an open input or repeated
references to one curve. Unavailable options reject atomically. Explicit batch
arguments preserve the stored flags; a flag without an eligible source has no
effect on the result. Shape gaps survive closed-contact movement.

Six terminal-zero private Xvfb captures retain 45 complete workflows, including
44 accepts, one cancellation preserving pending redo, and eight workflows with
17 intentionally rejected options. The corpus covers circles, a rational ellipse,
a spatial periodic rational curve, a rational cubic with a nonsmooth seam, a
closed polyline, point endpoints, repeated references and an ordinary curved
join. G0–G4, both contact roles, flips and shape/contact combinations are included.
Native outputs are expectations only; prescribed sources and public input
receipts construct the Rust results.

Portable and usd-brep builds check geometry, normalized derivatives, parameter
domains, source identity, attributes, undo/redo and 3DM round trips. Windowless
egui checks also use real pointer selection on a closed source, cached contact
previews, cancellation, flipping, and source replacement by an open curve.

Periodic parents expose two equivalent unused-knot conventions: RhinoCommon
pads its short vector by duplication, while OpenNURBS reconstructs superfluous
knots by extrapolation when loading 3DM. Their discrepancy is retained separately.
Controls, weights, every active knot, samples and derivatives keep their strict
gates; a mutation test ensures active-knot/control changes still fail. The fixture
loader mirrors the public Native input setter without reading expected outputs.

One initial eight-case pilot exited before publishing a response; a fixture setup
error also preceded capture. Neither contributes observations. Logs, frozen
producers and terminal receipts remain, and every pilot recipe repeats cleanly.

Geometry and normalized derivative gates remain `1e-6`; domains use
`max(2e-6, 1e-8 * native span)`. General closed surface-boundary frames, arbitrary
self-intersection/depth ties, complete Native drag/Escape trajectories, curvature
display, history reediting, continuous distance and comparative performance remain
unqualified. Earlier mathematical G3/G4 and long rational-domain limits remain.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_closed
```
