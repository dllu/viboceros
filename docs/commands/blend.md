# Blend

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/blend.htm)

Select two open curves and run `Blend`. It creates a new cubic NURBS curve
between the nearest selected ends. The source curves remain in place, the new
blend is selected, and `Undo` removes it. `Pick1=x,y,z` and `Pick2=x,y,z`
choose other source ends.

`Continuity1` and `Continuity2` independently accept `Position` or `Tangency`;
both default to `Tangency`. `Handle1` and `Handle2` set the respective cubic
handle lengths in model units. Each defaults to one third of the endpoint
distance.

```text
Blend Continuity1=Tangency Continuity2=Position Handle1=2.5
```

Curvature and higher continuity, surface-edge selection, interactive handle
adjustment, trimming, and joining are pending.
