# Maelstrom deformation foundation

[Architecture](architecture.md) · [Oracle](oracle.md)

`MaelstromPointMorph` implements the public
[MaelstromSpaceMorph](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_Morphs_MaelstromSpaceMorph.htm)
point map independently in Rust. This is the geometry and oracle foundation;
the application command, interactive circle construction and live previews
remain to implement.

## Point map

Construction takes a validated `Frame3`, fixed radius `r0`, rotated radius `r1`,
and an angle in radians. The frame origin and normal define the rotation axis.
Distance is measured in the frame's XY plane, ignoring axial height. Both radii
must be finite and exceed `2^-32`; the angle must be finite. This SDK validity
rule is independent of modeling tolerance. Invalid Rust constructors return an
error; the oracle adapter reproduces the SDK's unchanged points for invalid
definitions.

For unequal radii, with radial distance `r`, define:

```text
t = clamp((r - r0) / (r1 - r0), 0, 1)
rotation = angle * t² * (3 - 2t)
```

When `r0 < r1`, the inner region is fixed and the outer region receives the full
rotation. Reversing the radii fixes the outer region and fully rotates the inner
region. Between the radii, the cubic transition has zero endpoint slope.

Native near-equal radii use a separate, unbounded rule:

```text
abs(r1 - r0) <= 2^-26 * r0
rotation = angle * r / r0
```

The threshold uses the first radius. The retained boundary sweep distinguishes
this from symmetric thresholds and a rounded ratio comparison. Equal radii
therefore continue accumulating rotation beyond the specified circle.

The point map uses raw sine and cosine, preserving tiny rotations and residuals
at cardinal angles. Axis-aligned planes rotate world coordinates directly and
other planes add radial changes to the original point, preserving
axial coordinates even when their displacement cannot fit in binary64.
Compensated projections, scaled radius evaluation and exact rational fallbacks
recover representable angular products and final coordinates after intermediate
overflow or severe cancellation. Unrepresentable angular evaluations or final
points fail explicitly. The zero-angle map returns original points and NURBS
geometry without fitting.

## Geometry and verification

The shared `PointMorph` interface fits curves and surfaces with finite work
budgets, preserves mesh topology and point-cloud order, and supports B-rep
morphing. `with_preserve_structure(true)` maps existing Euclidean NURBS controls
while keeping degree, knots, weights and domains. Multi-face B-reps retain the
shared-edge fitting path. `rigid_transform(center)` reuses the measured forward
difference placement policy shared by Twist, Bend and Taper.

Four private Xvfb sessions on Rhino **8.32.26160.13001** retain 108 SDK cases and
eight actual point commands. They cover radius order, near-equal profiles at
five scales, exact SDK cutoff neighbors, normal vectors down to the smallest
positive binary64 value, spatial and translated planes, signed/tiny/multi-turn
angles, Copy, grouped Rigid, selection, and Undo/Redo. Comparison tolerances are
`1e-11 + 1e-12 * abs(native coordinate)` for SDK maps, `1e-11` for ordinary native
point commands, and `1e-7` for grouped rigid placement. The
[provenance record](maelstrom-provenance.json) hashes helpers and raw artifacts.

Independent Rust tests check extreme-range behavior, preserved rational controls,
identity structure, and 513 fitted-line samples within `2e-5` at a fitting
tolerance of `1e-5`. Fitting uses sampled checks; it is not a continuous error
certificate. Native full-geometry fitting, command preferences, circle options,
interactive workflows and performance comparisons remain unverified.

```sh
cargo test -p viboceros-geometry maelstrom::tests
cargo test -p viboceros-oracle maelstrom_points
python3 -m unittest tools.rhino_oracle.test_maelstrom
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/maelstrom_points.json \
  --observations tools/rhino_oracle/observations/maelstrom_points.json \
  --absolute-epsilon 1e-11 --relative-epsilon 1e-12
```

Public references: [command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/maelstrom.htm)
and [C++ point-map description](https://developer.rhino3d.com/api/cpp/class_c_rhino_maelstrom_space_morph.html).
No proprietary source was inspected.
