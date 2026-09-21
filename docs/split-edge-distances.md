# SplitEdge distance constraints

[Command](commands/split-edge.md) · [Provenance and hashes](split-edge-distance-provenance.json)

The [19 requests](../tools/rhino_oracle/fixtures/split_edge_distance_command.json)
and [unaltered result values](../tools/rhino_oracle/observations/split_edge_distance_command.json)
were measured using public Rhino 8.32 commands in owned private Xvfb sessions.
Every case includes real component selection and actual Undo/Redo. Additional
point clicks use bounded mouse targets, not simulated RhinoCommon GetPoint results.

## Measured behavior

Distance is arc length along the original edge. It persists after acceptance,
and each accepted point becomes the next reference. Typed coordinates honor the
constraint too. Negative input uses its magnitude, zero clears, and another
number replaces the distance. A sole reachable candidate is used even if the
cursor is on its opposite side. An unreachable distance leaves the batch intact.
Closed-circle edges wrap in either direction across the seam, but not beyond one
circuit. Source IDs, attributes, groups and one-command history are preserved.

## Independent accuracy checks

The quadratic edge is `C(t) = (30t, 30t(1-t), 0)`. Its signed arc length from
`a` to `b` is `15 * (F(1-2a) - F(1-2b))`, where
`F(u) = (u*sqrt(1+u*u) + asinh(u))/2`. Rhino's recorded 10-unit and 5-unit
distances have analytic residuals approximately `3.2e-8` and `4.8e-8`;
their chords are approximately `9.94946` and `4.97896`. The circle witnesses use
radius times the angle difference, independently of NURBS parameterization.
Their retained positions also have nonzero arc-length residuals above `1e-9`.

Native replay compares complete ordered spatial/UV definitions, topology,
attributes, selection and history. The twelve straight-edge cases use absolute
epsilon `1e-9`; the seven curved cases use `1e-6`, both with relative epsilon
`1e-10`. No numeric geometry fields or component permutations are discarded to
obtain agreement. Rhino-only event/history transcripts remain in the raw archive;
native replay compares the recorded before/after/Undo/Redo document states.
The original 21 unconstrained cases retain their original tighter bounds.
Kernel tests independently check the analytic parabola and circle more tightly
than the Rhino observation bound. Replay of a requested mouse location uses its
model point, not Rhino's quantized screen pixel, so this is not proof of arbitrary
camera or picking equivalence.

## Implementation and probe safety

The geometry kernel trims exactly at the anchor, reverses when needed, and
reuses adaptive arc-length integration and inversion. It does not add a short
distance to a large global prefix; a regression checks offsets after a `1e16`
prefix. Native-domain tests include very small, very large and signed domains.
The bounded kernel query never wraps; the command handles at most one closed
seam crossing, stopping at the original reference.

The command caches both reachable parameters when distance or anchor changes.
The constrained viewport path only projects those candidates, with no integration
or closest-curve search per frame. Computation errors leave the collected batch
and prior constraint unchanged. No cross-engine performance claim is made.

The oracle's alternative `inputs` grammar accepts up to 64 single-key objects:
`{"point": native_parameter}`, `{"mouse": native_parameter}`, or
`{"distance": length}`. It is exclusive with the original `parameters` grammar.
Point values are evaluated on the original edge. Distance values are finite
numbers, never arbitrary command text. A UI-thread timer requests each later
owned click only after its corresponding `_Pause` appears in the command history.
It excludes the first component-selection pause, emits no duplicate requests,
fails closed on history discontinuity or I/O errors, and always detaches/disposes.

Other-object snapping, no-anchor distance entry and equal-distance tie policy
remain unmeasured; the native command requires a previously accepted edge point.
Extreme-domain integration failures are reported, not silently approximated away.
