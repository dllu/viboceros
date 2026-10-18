# Curve arrays and Flow direction units

`ArrayCrv` rebuilt validated unit frames using the document's distance
tolerance, rejecting all rotating orientations at large model scales. Those
axes now remain typed as unit directions; angular admission is unchanged.

Flow also treated short line primitives through a selection predicate with a
fixed minimum feature length. Validated line primitives now take the linear
path independently of model units. The existing NURBS predicate remains in use.
For circle, arc and ellipse guides, Flow checks a generic support-plane result
against the known analytic plane. It retains the native plane orientation when
they agree and uses validated metadata when the generic query has mistaken a
small conic for a line or cannot supply its plane.

Opposite linear guides retain the existing half-turn-plane rule: their
separation determines the plane; exact collinearity uses the deterministic
perpendicular. Normal construction now has an ordinary floating-point path and
an exact range-recovery path. Point differences must be exact before using the
fast path. Otherwise determinants are accumulated with the existing rational
scalar helpers and scaled before conversion to unit coordinates. Overflowing
origin differences and underflowing cross products no longer select a false
collinearity branch or erase a tiny transverse offset.

| Original matrix | Cases | Before successful | After successful |
| --- | ---: | ---: | ---: |
| Curve arrays | 480 | 336 | 480 |
| Opposite-line Flow maps | 48 | 36 | 48 |
| Overflowing-origin Flow points | 2 | 0 | 2 |

Array cases use scales `2^-20`, `1`, `2^20`, `2^40`, `2^80`; absolute tolerance
is `1e-7` times scale. They cover an arc, two NURBS arc parameterizations with
domain widths `2^-200` and `2^200`, vertical/oblique lines and a spatial polyline
with a vertical segment. All four orientation modes, item/distance spacing and
explicit/default base points are exercised. Analytic station positions,
rotation/tangent/heading rules, unit axes and handedness are checked alongside
copy counts, source selection, group identities and exact Undo/Redo replay.

The opposite-line matrix uses scales from `2^-600` to `2^600`, three separation
planes and Stretch on/off. Expected points use the analytic half-turn formula,
with normalized errors below `2e-12`. A separate pair of witnesses has origins
at opposite `1e308` coordinates and a transverse offset of only `1e-200`; its
finite half-turn map must retain the exact expected coordinate signs.

Further regressions cover:

- Ten line-to-tilted-arc Flow cases, including short lines borrowing the correct
  target plane, with normalized point error below `2e-11`.
- 96 Flow command cases with Copy/Stretch, groups and object replay. Existing
  empty copied-group definitions survive Undo; Redo restores their memberships.
- 25 array admission/no-op cases preserving object state and redo history.
- Four kernel tests with 25 normal queries, covering raw product scales from
  `2^-2000` to `2^2000`, subnormal products, overflowing origin differences,
  reversed normals and exact collinearity hidden by rounded point subtraction.

The command/Flow matrices total 661 cases, in addition to the four kernel tests.
Array station errors are bounded by `2e-7` after normalization; axis-length,
orthogonality and orientation checks use `2e-10` bounds. Inputs remain unchanged
when copied, and array errors preserve their existing state and history.

The [validation record](curve-direction-validation.json) retains original
baseline test sources, complete logs and exact source hashes. No new Rhino
capture or performance measurement was made; retained native observations are
still exercised by the full suites. These cases do not qualify arbitrary
scales, every NURBS guide's plane/line classification, general spatial Flow
policy, topology or viewport parity.

```sh
cargo test -p viboceros-geometry --test flow_direction_units
cargo test -p viboceros-command --test curve_array_units --test flow_direction_units
cargo test --release -p viboceros-geometry -p viboceros-command --features viboceros-command/native-smlib
cargo test --release --bin viboceros
```
