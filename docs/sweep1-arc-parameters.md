# Sweep1 arc parameters

[Sweep1 placement](sweep1-placement.md) · [Validation](sweep1-arc-parameters-validation.json)

Automatic sweeps now retain the measured Rhino command behavior for analytic arc
stations. The nearest-point query still certifies a minimum-distance witness on
the complete rational source curves. The command adapter then converts the arc
witness to its native analytic parameter and uses that value in the rational
transport rail. Other geometry parameter maps and explicit `Parameters=` scripts
retain their existing meanings.

This policy is inferred from public Rhino 8.32 command outputs. On a radius-three
quarter arc, a radial profile at thirty degrees ends at approximately
`(-0.0378230828, 2.9997615596, 0)` in Rhino. Using the analytic station directly in
the rational chart predicts that endpoint within `7.1e-16`. A geometrically
equivalent NURBS rail can therefore give a different automatic sweep.

`RefitRail=Yes` begins the swept interval at the profile, including a profile at
an existing Greville station. With `RefitRail=No`, an existing station can anchor
the complete rail. Near-station decisions on some multi-span rational inputs
remain under investigation; this is not complete Rhino sweep compatibility.

## Complete-image checks

The independent holdouts cover 36 command outputs: analytic and equivalent
rational arcs, both refit settings, translated/rotated planes, reversed traversal,
different radii, and profiles with a normal component. The original thirty-degree
input has its own regression. Construction tolerance is `1e-7`; the continuous
image comparison uses `1e-6`.

All captured faces participate. Each surface is affine in normalized V because
its two positive weight rows match. Both complete U boundary atlases are compared
under the same monotone parameter correspondence. Full-order join knots preserve
every captured face and both one-sided limits. Exact rational curve bounds then
extend to the entire surface images by convexity in V. Numerical local searches
only propose correspondence nodes. Proofs are reused only when both actual
surface atlases and complete captured values are exactly equal.

Different parameter clocks and face partitions are permitted by this image
comparison. It does not certify identical topology, surface derivatives or UV
parameterization. The separate 28 interval and 20 knot/Greville captures are
investigation records; their surface interiors are not qualified by the arc test.
They retain the remaining rational near-Greville behavior and spatial sweep or
closest-point discrepancies for further work.

```sh
cargo test -p viboceros-command --lib sweep::arc_station_tests --frozen
python3 -m unittest tools.rhino_oracle.test_sweep1_arc_parameters
```
