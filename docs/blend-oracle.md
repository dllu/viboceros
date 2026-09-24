# Curve blend API comparison

[Blend command](commands/blend.md) · [Oracle setup](oracle.md)

The [five shared line inputs](../tools/rhino_oracle/fixtures/blend_lines.json)
were run against Rhino 8.32's public
[`Curve.CreateBlendCurve`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/createblendcurve)
API in an owned private Xvfb session. The
[raw response](../tools/rhino_oracle/observations/blend_lines.json) records the
full degree, control points, weights, knots, and parameter domain. This is an
API comparison; it does not prove identical interactive `Blend` command input
or handle behavior.

For these line inputs, Rhino returns a degree 1 line for G0, a degree 3 blend
with each G1 handle equal to the endpoint distance, and a degree 5 blend with
each G2 handle 40% of that distance. Viboceros now matches every degree and
control point within `1e-12`. Rhino's curve domain ends near its computed arc
length; the saved raw domains differ from Viboceros's independently integrated
lengths by less than `3e-8`. The [oracle regression](../crates/viboceros-oracle/src/blend_curve.rs)
checks these bounds without rewriting the recorded Rhino values.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/blend_lines.json --timeout 600
cargo test -p viboceros-oracle --lib blend_curve::tests
```
