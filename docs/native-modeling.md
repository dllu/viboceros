# Native modeling implementation guides

[Documentation](README.md) · [Architecture](architecture.md) · [Build and tests](usd-brep.md)

The optional `native-smlib` feature uses usd-brep/SMLib for native B-rep modeling.
These pages record integration details and qualification limits. Start with
[USD BRep integration](usd-brep.md) and [current limitations](usd-brep-limitations.md).

- [native B-rep modeling](smlib-import.md)
- [paired native isocurves](native-isocurve-pairs.md)
- [original parent UV charts](fillet-parent-charts.md)
- [Automatic surface domains](automatic-surface-domains.md)
- [Gordon networks](smlib-gordon-network.md)
- [automatic curve fitting](automatic-gordon-network-fit.md)
- [Native curve/surface endpoint evaluation and one-sided derivatives](native-basis-evaluation.md)
- [interpolation at supplied parameters](native-point-interpolation.md)
- [exact curve/surface splits](native-exact-splits.md)
- [tensor Bezier decomposition](native-tensor-bezier.md)
- [USD BRep kernel integration](usd-brep.md)
- [Native surface-to-surface fillet producer](native-surface-fillets.md)
- [UV pick sides and construction accuracy](native-surface-fillet-picks.md)
- [parent trimming/splitting](native-surface-fillet-parents.md)
- [Circular fillet rail correspondence](native-circular-fillet-rails.md)
- [Native surface joining](smlib-joining.md)
- [UV transposes](native-uv-transpose.md)
- [closed-component orientation](native-closed-join.md)
- [Native pole transfer and exact NURBS copies](native-pole-transfer.md)
- [Native surface-normal pulling foundation](smlib-surface-pull.md)
- [USD BRep limitations and bugs](usd-brep-limitations.md)
- [Native NURBS offsets and qualification](smlib-curve-offset.md)
- [Native curve intersections and qualification](smlib-curve-intersections.md)
- [Native in-tolerance curve contacts](native-curve-contact-tolerance.md)
- [Native UV drop accuracy](native-drop-curve-accuracy.md)
- [Native curve self-intersections and qualification](smlib-curve-self-intersections.md)
- [Native curve/surface intersections and qualification](smlib-curve-surface-intersections.md)
- [Native general surface intersections, face trims and tangencies](smlib-surface-surface-intersections.md)
- [Native lamp-well conversion regression](native-lamp-well.md)
