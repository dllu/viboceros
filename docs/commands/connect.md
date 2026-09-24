# Connect

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/connect.htm)

Select two open curves and run `Connect`. The selected ends are trimmed or
extended to meet at one point. Two updated curves are selected, and one Undo
restores the originals. `Join=Yes` creates a single selected polycurve.

The nearest pair of endpoints is used by default. `Pick1=x,y,z` and
`Pick2=x,y,z` choose other ends. Separate outputs inherit their respective
sources' attributes and groups; a joined output inherits the first source's.

Straight terminal segments may meet by extension. Nonmeeting NURBS and
polyline ends can extend with straight tangent segments; this is also available
as `ExtendOtherCurvesBy=Line`. Curved terminal segments are also supported when
their selected endpoints already meet.
For coplanar arc/line pairs, the default `ExtendArcsBy=Arc` extends the arc on
its supporting circle to a valid line intersection. Coplanar arc pairs can
extend both arcs to a supporting-circle intersection. `ExtendArcsBy=Line` adds a
straight tangent extension instead. Other curved-pair extensions and the
`ExtendOtherCurvesBy=Smooth` option remain pending.
