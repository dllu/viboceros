# Curved-face BlendCrv G3/G4

[Command guide](commands/blend-curve.md) · [Alt editing](blend-curve-alt.md) · [Validation record](blend-curve-surface-high-validation.json)

Regular supporting faces now accept G3/G4 constraints alongside the existing
contacts, numeric angles, shape handles, Shift symmetry and mixed-source joins.
Surface contact UVs update when a contact moves; handle-length candidates reuse
them without repeating a global surface closest-point search.

The geometry kernel evaluates all nine first/second/third partials of a NURBS
surface analytically. Active-span homogeneous derivative nets feed the rational
quotient recurrence, including nonzero rational derivatives above the polynomial
degree. Signed cancellation, continuation and intermediate range loss recover
from exact original binary64 inputs before final rounding. One-sided limits
and extended evaluation are exposed through the third-derivative API.

The command follows Rhino's measured policy: take the straight UV line initially
tangent to the blend, normalize its third target to remove tangential parameter
acceleration, then project it onto the surface normal. This differs from a
geometric normal section on nonorthogonal faces. Thirty-six independent target
comparisons distinguish the policies. G4 keeps the measured zero-fourth target;
a quintic source with normalized fourth normal derivative `1.152` demonstrates
that this is not general mathematical fourth-order contact.

Three successful private Xvfb captures retain 40 new workflows. Two earlier
curved-face diagnostics are promoted, giving 39 fully matched command/editor
workflows and three long-handle geometry replays per build: 41 accepts and one
cancellation preserving pending redo. The corpus covers cubic/quintic/rational faces, nonorthogonal charts, curved boundaries,
shifted/scaled UV domains, angles, reversed and mixed inputs, points, edits,
joins, Top/Perspective views, source identities, history and 84 3DM round trips.
Nearly planar G2/G3/G4 faces retain tiny derivatives that long handles amplify;
the command no longer flattens faces using a planarity tolerance. Those three
2000-unit-handle cases exceed the unchanged parameter-domain gate and remain
explicit domain diagnostics. Their geometry, derivatives, history and archives
pass; Rust lengths agree within `1e-7` with independent integration of the captured
Native curves. Both failed regression receipts are retained.

Seventy-six passive public `Surface.Evaluate` stations independently check all
third partials. Native output geometry never seeds the Rust constructions.
The full kernel run passes 2,852 tests; ten manual timing/export tests retain
their repository ignore markers and are listed in the validation record.

One failed capture opened an ambiguous-edge menu. Its terminal receipt, frozen
producers, progress log and private screenshot are retained. The retry rigidly rotates the prescribed surfaces and picks to separate their
projected edges, and corrects the authored opposite-edge contact. Only derived source-pick coordinates permit up to eight
ulps of CPython/IronPython weighted-sum rounding; source definitions, options
and view settings remain exact, and physical pixel/ray/face receipts are kept.

Geometry and normalized blend derivative gates remain `1e-6`; independent
surface partials use a `1e-8` relative gate. Thirty-nine workflows retain the
earlier `max(2e-6, 1e-8 * native span)` parameter-domain gate. Mathematical G3/G4
contact on arbitrary parameterizations, Native held Alt dragging, shared/seam faces,
trimmed/closed boundaries, singular frames, arbitrary ambiguous picking,
continuous distance, comparative performance and exact Native approximate-domain
parity remain unqualified.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_surface_high
```
