# PlanarSrf open boundaries

`PlanarSrf` joins selected curve pieces before constructing exact planar faces.
Lines, polylines and rational arcs can form outer loops, holes and islands.
Shuffled and reversed pieces use the existing batch joiner's traversal and seam
rules. Source geometry remains unchanged. The curve selection prompt also
supports picking inputs after starting the command and setting `DeleteInput`.

Native observations distinguish complete joined loops from recovered portions
of an incomplete walk. Complete loops use the ordinary seam-based region
planner. Repeated junctions can recover linear or rational closed walks from
tails and bridges. A line-chain endpoint can also restrict a nonadjacent span
when its projection lies strictly inside that span and its residual is less
than document tolerance. Closure depends on direction: a small perpendicular
gap can fail even when a larger gap along a span can be restricted.

When a recovered loop participates, the observed command fills the union of
outlines on the first complete profile's plane, or the first recovered plane
if none is complete. This can fill existing nested holes. A stray line or
incomplete chain without a recovered loop preserves ordinary hole behavior.
These rules describe the captured scope; arbitrary self-intersections and
rational endpoint-to-interior recovery need further qualification.

Assembly limits input and derived controls to 100,000, admitted profiles to
128, and charged traversal/source comparisons to one million each. Source
membership for `DeleteInput=Yes` uses a conservative spatial index and checked
positive overlap intervals; point contacts do not consume unused sources.
Geometry staging finishes before document edits, including failure paths.

The [validation record](../planar-chains-validation.json) covers 136 native
workflows. In each build, 135 match command status, source membership, source
definitions, areas and Undo/Redo. Successful qualified outputs also pass
bidirectional spatial boundary samples, 3DM round trips and normal-mesh
generation. Boundary samples use every nonzero span at 17 stations, with a
`1e-6` model gate; this is not a continuous error certificate or native output
encoding/triangulation parity. Application tests repeat the corpus without
opening a window; prompt tests cover picking, deletion and cancellation.

One explicit gap remains: `chains_filled_outside_tangent` succeeds in Rhino
(area 116), while the exact Boolean exporter declines its narrow tangent
sliver. The comparison requires that decline and verifies unchanged objects
and history. It is not counted as qualified compatibility. Comparative speed
and broader branch configurations remain unqualified.

```sh
cargo test -p viboceros-oracle --test planar_chains -- --nocapture
cargo test -p viboceros-command --test planar_chains
cargo test -p viboceros planar_chains
python3 -m unittest tools.rhino_oracle.test_planar_chains
```

Repeat Rust checks with `--features native-smlib` for the usd-brep adapter.
Native captures use the private Xvfb launcher; no desktop windows are needed.
