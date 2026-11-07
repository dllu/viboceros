# BlendCrv higher controls

[Command guide](commands/blend-curve.md) · [Validation record](blend-curve-controls-validation.json)

Every G2–G4 Bézier control can be selected and moved along its endpoint tangent.
The primary length stays fixed. Each higher tangent gap is stored relative to
that length, so moving an earlier control shifts later controls and resizing the
primary scales all gaps. Negative gaps are allowed. Contact and surface-angle
changes retain these ratios. Changing continuity resets that endpoint's shape.

Shift copies the selected end's complete shape to the other end when both
continuities match. Alt on a higher surface control rotates the supporting-face
direction. Press modifiers last for that adjustment. A higher-control numeric
entry sets a positive distance and waits for a point to choose direction; its origin is the second
control's tangential position, even when editing the third or fourth control.
`Spacing1=r2,r3,r4` and `Spacing2=r2,r3,r4` preserve ratios in scripted commands.
Defaults are `1,1,1`; these dimensionless options supplement `Handle1/2`.

The kernel composes normalized endpoint derivatives through order four. With
unit tangent T, curvature K, normal third target J and scalar freedoms a,b,c,
G2/G3 use `K+a*T` and `J+3a*K+b*T`. Measured G4 control edits use
`6a*J + (12a*(b+K.K)+9a³)*K + c*T`. This differs from mathematical scalar
composition, which remains available through a separate kernel API. For finite
endpoint speeds, exact rational fallbacks protect overflowing, underflowing or
cancelling intermediate products when the resulting derivative is representable.
An overflowing endpoint speed returns a geometry error.

Eight terminal-zero private Xvfb captures retain 80 observations. Three early
numeric investigations timed out before direction confirmation and remain
diagnostics. With the earlier secondary-control observation, 77 geometry/history
replays cover planes, curved/spatial faces, ordinary/nonplanar/rational curves,
point endpoints, reversed ends, symmetry, continuity changes, joins and pending
redo. Native outputs are expectations and never construct the Rust result.
There are 76 workflows within the full gates. One long rational-source blend
passes geometry/history checks but exceeds the unchanged domain gate by a factor
of 4.55. A measurement-only repeat shows that the command domain equals Native's
default public SDK length; tighter SDK tolerances converge toward independent
integration, which agrees with Viboceros. This remains an explicit domain
diagnostic alongside the original failed check.
Portable and usd-brep builds check geometry, normalized derivatives, parameter
domains, identity, attributes, undo/redo and 3DM round trips. Windowless egui tests
also cover held drags, press modifiers, numeric confirmation, cancellation and
invalid candidates.

Geometry and normalized derivative gates are `1e-6`; approximate Native domains
use `max(2e-6, 1e-8 * native span)`. Complete Native drag/Escape trajectories,
arbitrary occlusion/depth ties, shared/seam/trimmed/closed surface boundaries,
singular frames, mathematical general G3/G4 continuity, continuous distance and
comparative performance remain unqualified.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_controls
```
