# Blend

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/blend.htm)

Select two open curves and run `Blend`. It creates a new NURBS curve
between the nearest selected ends. The source curves remain in place, the new
blend is selected, and `Undo` removes it. `Pick1=x,y,z` and `Pick2=x,y,z`
choose other source ends.

`Continuity1` and `Continuity2` independently accept `Position`, `Tangency`, or
`Curvature`; both default to `Tangency`. Two position ends produce a line;
tangency blends are cubic;
curvature at either end produces a quintic that matches the source curvature
at that end. `Handle1` and `Handle2` set the respective handle lengths in model
units. The default length is the endpoint distance for tangency and 40% of it
for curvature. Position handles default to one third of that distance when
combined with another continuity mode.

The [Rhino API comparison](../blend-oracle.md) covers the default G0, G1, and
G2 shapes for five line input cases.

```text
Blend Continuity1=Tangency Continuity2=Position Handle1=2.5
```

G3 and higher continuity, surface-edge selection, interactive handle adjustment,
trimming, and joining are pending.
