# Planar cut arithmetic

[Planar commands](commands/planar-booleans.md) · [Numerical policy](numerical-robustness.md)

The mixed straight/circular Boolean path forms cut coefficients from exact
rational representations of the stored projected binary64 endpoints. Endpoint
differences are exact before determinant products. Line crossings use exact
signs and interval tests, then round each normalized station once. Collinear
overlap retains both directed intervals; an endpoint contact is a point rather
than a positive-length overlap.

Line/circle cuts use exact quadratic coefficients and discriminant signs.
Binary exponent normalization keeps the square root finite when its square
would overflow or underflow. The root farther from zero is computed first;
Vieta's product recovers the smaller root without subtractive cancellation.
Exact endpoint roots are factored before rounding. A nonzero station that rounds
to an endpoint, or distinct admitted cuts that round to the same station, fails
with `UnrepresentableBrepBoolean` rather than losing topology. Rational sizes
remain subject to the Boolean arithmetic budget.

An independent Python `Fraction` reference supplies 206 line-pair cases with
bit-for-bit station comparisons. Regression cases include determinant
cancellation to zero, exponents from `-550` to `600`, reversed overlaps,
contacts, tangency, quadratic cancellation near an endpoint and unrepresentable
cut separation. A caller regression checks that line stations are retained
directly without a point/closest-parameter round trip. Reproduce the corpus with:

```sh
python3 tools/numerics/generate_planar_cut_reference.py > /tmp/planar-cut-reference.json
cmp /tmp/planar-cut-reference.json crates/viboceros-geometry/src/brep/boolean/planar_mixed/cuts/reference.json
```

Circle pairs share one exact-coefficient helper across the complete-circle and
mixed-boundary paths. It computes `(d^2 + r^2 - s^2) / (2d)` and the squared
perpendicular offset rationally before final rounding. This avoids cancellation
from nearly equal squared radii and recovers heights when squared terms exceed
binary64. The center distance `d` is still a rounded binary64 length. Coincidence
and tangency bands retain the document's absolute tolerance policy, with exact
comparisons against the stored scalar values.

A second independent reference has 172 Fraction/Decimal cases. Longitudinal
offsets match bit for bit; heights are within one binary64 step of 180-digit
Decimal square roots. It includes binary scale exponents `-550` through `600`
and near-internal contacts with radii from `2^20` to `2^50`. A concrete regression
removes a `6.9e-4` height error from the previous squared-radius subtraction.
Swapping radii preserves height, and finite/positive input validation remains
explicit. Generate the reference with:

```sh
python3 tools/numerics/generate_circle_cut_reference.py > /tmp/circle-cut-reference.json
cmp /tmp/circle-cut-reference.json crates/viboceros-geometry/src/brep/boolean/circle_cut_reference.json
```

Independent analytic segments can evaluate a common junction to slightly
different coordinates. The general `PolyCurve3` fixed-coincidence contract stays
unchanged. Boolean export checks every junction at the document tolerance;
where fixed coincidence fails, it clamps rational segments and shares midpoint
endpoint controls. Each endpoint moves at most half the admitted gap. With
positive weights the pointwise displacement stays within the largest endpoint
edit. A regression checks sampled displacement and rejects gaps exceeding the
document tolerance. This is a bounded final export edit, not exact preservation
of the analytic circle locus.

The [scale capture](../tools/rhino_oracle/observations/planar_boolean_scale.json)
contains 27 successful owned Rhino commands under private Xvfb with scheme
`VibocerosOraclePlanarScale20261008`. It covers disk/rectangle side cuts, strips
and polygon-first operands for all three commands at scales `2^-20`, `1` and
`2^20`, using absolute tolerance `1e-7 * scale`. Command replay checks topology,
identity, attributes, groups, independent history and bidirectional finite curve
witnesses at `5e-6 * scale`. App replay checks picking and history for every case.
Local total area divided by `scale^2` agrees with analytic formulas at `1e-9`.

Native mass integration differs from those formulas by up to `2.103e-4` after
normalization; native source disks differ by up to `1.222e-4`. These measurements
are retained separately. Only this scale capture uses normalized native area
bounds `3e-4` for outputs and `2e-4` for sources; earlier capture tolerances stay
unchanged. [Provenance](planar-boolean-scale-provenance.json) binds the complete
capture, producer, recipes and independent reference.

The [circular scale capture](../tools/rhino_oracle/observations/planar_circle_scale.json)
adds 27 successful commands under `VibocerosOracleCircleScale20261008`, covering
equal/unequal radii and swapped operands at the same three scales. All local
normalized areas match analytic disk-segment formulas at `1e-9`. All cases replay
face/edge counts, source identity, attributes/groups and independent history;
application replay also checks the full picking/history workflow.

Six equal-radius cases at scales `1` and `2^20` meet the existing normalized
bidirectional boundary bound `5e-6`. The other 21 remain explicit diagnostics:
unequal-radius native fits differ by up to `8.632e-6` at these scales, and the
smallest-scale native results differ by up to `0.004588` in bidirectional finite
witnesses. Their native circle-locus deviation reaches `0.004060`, normalized
area error `0.014638`, and source disk area error `0.000275`. Tests reproduce
the individual diagnostic measurements rather than relaxing the agreement
bound. Native output/source area checks at scales `1` and `2^20` retain `2e-5`
and `5e-7`; small-scale native areas are recorded diagnostics. Earlier captures
retain their tolerances. [Circular provenance](planar-circle-scale-provenance.json)
binds the full inputs/outputs and both reference and diagnostic evidence.

Exact cut coefficients do not make the entire geometry pipeline exact.
Projection, arc station recovery, membership, contour assembly and export still
use binary64. Primitive extreme-exponent tests do not establish whole-model
support at those scales. Finite curve witnesses are not continuous boundary
certificates. General curved Booleans, near contacts and relative performance
remain unverified.
