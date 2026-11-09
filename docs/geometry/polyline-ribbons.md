# Ordinary polyline ribbons

[Geometry guides](README.md) · [Validation record](../ribbon-polyline-validation.json)

`try_polyline_ribbon` builds ordinary planar polyline offsets with Sharp, Round,
Chamfer or None corners. It returns edge-connected B-rep components. BothSides
sews the two sides at original source edges. The signed distance chooses the
first side, and U spans `[0,abs(distance)]`. Segment V parameters retain the
source chart; outer fan parameters follow their offset arc or line.

Construction reuses the ordinary offset layout. Each original segment retains
its own offset correspondence. Round and Chamfer outside corners add a fan
with a constant source boundary; rational round fans use the same weights on
both rows. Inside corners split into strips and triangles with constant offset
boundaries. None omits outside fans. The `_with_ruled` variant delegates tensor
construction to usd-brep through `viboceros_smlib::ruled_surface`.

The local Python operation `polyline_ribbon_geometry` accepts `vertices`, signed
`distance`, `corner`, optional `parameters`, `both_sides` and `normal` (default Z).
It returns complete `breps`, unchanged source readbacks and false flags for a
constant-distance certificate and full command parity. It rejects unsupported
Smooth corners, invalid distances, nonplanar inputs and collapsed offsets. A
conservative 4096-patch allocation guard bounds the work.

Native holdouts cover acute/obtuse turns, zigzags, closed loops, both sides,
reversed traversal, tilted planes and native PolylineCurve inputs. All 52 public
Ribbon workflows ran in one private Xvfb session with option-history checks.
Complete tensor bounds use `1e-9`; samples do not admit comparisons. Validation
also checks full rectangular native trims, edge/vertex geometry, face-loop edge
incidence and support-surface 3DM round-trips where qualified.

Both builds qualify 50 complete images and 42 incidence comparisons. Sharp
zigzags retain different face splits (3/6 local faces versus 5/10 native);
their complete native image is unproved. Eight one-sided None cases have
separate local vertices where native faces share a source vertex. All 52 local
support-surface archive round-trips pass. Face reversal flags agree in 31 cases;
19 both-sided cases differ and two mappings are unavailable. Loop winding,
object order and native output charts are not qualified.

`Ribbon` remains unregistered. Smooth curves/corners, self-intersection trimming,
command getters, metadata/history and performance parity remain unfinished.
