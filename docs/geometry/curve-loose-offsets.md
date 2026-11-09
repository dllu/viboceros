# Polynomial loose curve offsets

[Geometry guides](README.md) · [Validation record](../curve-loose-offset-validation.json)

`NurbsCurve::try_loose_polynomial_offset(distance, normal, tolerance, solver)`
offsets original Greville positions along planar unit normals and interpolates
them in a polynomial version of the original basis. Distance is signed. The
normal supplies the orientation of the automatically determined source plane.
The optional edit-point solver uses usd-brep in native builds.

This is a loose approximation: constant distance between stations is not
certified. Rational source positions and tangents are evaluated before weights
are replaced. Interior knot samples use the right tangent. A closed curve with
a kinked seam can produce an open offset, matching the captured native behavior.
Periodic sources use cyclic interpolation before clamping; other sources clamp
to their active domain. Existing edit-point limits apply: degree at most 17,
1000 stored controls, and common-sign weights. Failures preserve the source.

`try_loose_planar_offset` retains its earlier rational weights and sided edit
stations for `Inset`. The two policies share sampling and interpolation support.
The independent native Inset regressions remain unchanged.

For Python/JSON instrumentation, use `curve_loose_offset_geometry` with `curve`,
signed `distance`, and optional `normal` (default `[0,0,1]`). It returns `curve`,
unchanged `before`/`after`, and `distance_certified=false`. Returned definitions
can be reused directly. This local operation preserves the source parameter
domain; native output domains may be rescaled. Comparisons use normalized charts.

Both builds reproduce 29 complete native offset curves from 26 public command
workflows at `1e-9`. They cover lines, analytic circles, rational and polynomial
curves, polyline/polycurve runtime types, periodic and unclamped bases, nonuniform
knots, tilted planes, degrees 1–4, gauges, and both signs. Both-sided outputs are
matched as a set; object order, attributes, selection and history are not claimed
by this kernel comparison. One additional complete curve bound qualifies the
outer boundary of a native loose Ribbon.

The [Ribbon reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/ribbon.htm)
describes offset and ruled-surface construction. Thirteen corrected headless
captures retain its actual corner faces and both-sided topology for subsequent
implementation. `Ribbon` remains unregistered, and `Offset` does not yet expose
this new loose policy. Curve fitting between stations, broader corner/trim
policies, command getters and native performance parity remain separate work.
