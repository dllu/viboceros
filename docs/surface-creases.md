# Adaptive surface crease checks

[Loft](loft.md) · [Geometry architecture](architecture.md)

`Brep::try_surface_partitioned_at_discontinuities` supplements the public
OpenNURBS continuity query with an adaptive G1 check. The basic query samples
transverse span ends and midpoints and can miss a bend between them.
`NurbsSurface::discontinuity_parameters` retains that raw query's semantics.

At each remaining C0 candidate knot, the partitioner extracts only the two
adjacent local spans through the existing homogeneous Bézier extractor. It
builds the two directional derivative numerators in a shared Bernstein basis.
Positive coefficient combinations inside an acute angular cone screen smooth
intervals; unresolved intervals subdivide and provide evaluated angle witnesses.
Local coordinates and common weight scales avoid unnecessary world-offset and
projective-gauge cancellation. Bounds composition and circle recognition now
share the same coefficient-product utility.

G2 and Gsmooth requests also receive this G1 check; their higher-order tests
retain the existing query. Faces keep exact source restrictions, native charts
and sewn shared edges. Source tensors remain unchanged. The shared 16-million
work budget and a 64-level subdivision limit return an error if a decision
cannot be resolved. Floating roundoff, angular-limit decisions, degeneracies
and arbitrary signed-weight cases remain unqualified; this is not a formal
continuous-angle certificate.

Four frozen Rhino 8.32 captures contain 117 records. Geometry comparisons check
coefficients, knots, domains and 289 points per face at absolute `1e-9`, with
relative `1e-12` for weights under common gauges from `1e-120` to `1e120`.
They include rational surfaces, exchanged axes, shifted domains and coordinates,
and model scales from `1e-3` to `1e3`.

There are 98 complete raw topology matches and 19 additional exact partitions.
Rhino's own one-sided derivatives confirm narrower bends above the requested
angle where its splitter retains one face: one measured angle is about 0.431°
at a 0.1° limit; rational witnesses exceed a 0.5° limit. The adaptive result
retains the same surface with an additional shared edge. The full raw comparison
remains an explicit ignored diagnostic, including its original native face counts.

The Straight Loft Rebuild crease difference is resolved. All 60 open records
in the retained 140-command corpus now pass ordinary regressions; the remaining
full-command differences concern closed-solid face sense.

The [source and measurement record](surface-creases-validation.json) gives
numeric bounds and the exact comparison scope. Reference methods are the public
[surface continuity query](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_GetNextDiscontinuity.htm)
and [B-rep crease splitter](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Collections_BrepFaceList_SplitKinkyFaces_2.htm).

```sh
cargo test -p viboceros-geometry --test surface_creases_native
python3 -m unittest tools.rhino_oracle.test_surface_creases
# Retained topology differences:
cargo test -p viboceros-geometry --test surface_creases_native -- --ignored
```
