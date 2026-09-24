# Blend

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/blend.htm)

Select two open curves and run `Blend`. It creates a new NURBS curve
between the nearest selected ends. The source curves remain in place, the new
blend is selected, and `Undo` removes it. `Pick1=x,y,z` and `Pick2=x,y,z`
choose other source ends. Alternate end picks use Rhino's endpoint-specific
default blend shape and a normalized `[0,1]` parameter domain.

`Continuity1` and `Continuity2` independently accept `Position`, `Tangency`, or
`Curvature`; both default to `Tangency`. The blend uses the minimum degree for
the requested endpoint constraints: degree 1 for position/position, degree 3
for tangency/tangency, degree 5 for curvature/curvature, and the intervening
degrees for mixed choices. Curvature ends match the source curvature vector.
`Handle1` and `Handle2` set handle lengths in model units. Equal-continuity
defaults use the endpoint distance for tangency and 40% of it for curvature.
Mixed blends use Rhino-calibrated defaults from both source tangents when
available. `Bulge1` and `Bulge2` multiply the default handle lengths; each
defaults to 1. An explicit handle length replaces the bulge at that end.

The [Rhino API comparison](../blend-oracle.md) covers 282 line, arc, polynomial
or rational NURBS, polyline, and polycurve inputs, including nonparallel spatial
cases, alternate endpoint picks, and bulge factors.

```text
Blend Continuity1=Tangency Continuity2=Position Handle1=2.5
Blend Bulge1=0.5 Bulge2=1.5
```

G3 and higher continuity, surface-edge selection, interactive handle adjustment,
trimming, and joining are pending.
