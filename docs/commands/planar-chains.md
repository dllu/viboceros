# PlanarSrf open boundaries

`PlanarSrf` joins selected curve pieces before constructing exact planar faces.
Lines, polylines and rational arcs can form outer loops, holes and islands.
Shuffled and reversed pieces use the existing batch joiner's traversal and seam
rules. Source geometry remains unchanged. The curve selection prompt also
supports picking inputs after starting the command and setting `DeleteInput`.

Native observations distinguish complete joined loops from recovered portions
of an incomplete walk. Complete loops use the ordinary seam-based region
planner. Repeated junctions can recover linear or rational closed walks from
tails and bridges. Open self-crossings can also recover bounded regions.
A line-chain endpoint can restrict a nonadjacent span
when its projection lies strictly inside that span and its residual is less
than document tolerance. Closure depends on direction: a small perpendicular
gap can fail even when a larger gap along a span can be restricted.

When a recovered loop participates, the observed command fills the union of
outlines on the first complete profile's plane, or the first recovered plane
if none is complete. This can fill existing nested holes. A stray line or
incomplete chain without a recovered loop preserves ordinary hole behavior.
The filled mode traverses the whole source network, including spans omitted
from an individually recovered loop. Dangling branches and bridges do not add
area, and nested outlines are covered once. An attached U path contributes to
this mode; an isolated U path does not activate it. Complete joined loops retain
the ordinary seam planner. Broader self-intersections and rational endpoint
recovery need further qualification.

Assembly limits input and derived controls to 100,000, admitted profiles to
128, and charged traversal/source comparisons to one million each. Source
membership for `DeleteInput=Yes` uses a conservative spatial index and checked
positive overlap intervals; point contacts do not consume unused sources.
The filled graph also uses a 64 MiB storage estimate and a 16-million curve-pair
limit. Local Bezier extraction avoids repeated whole-spline trimming. Straight
line cuts and ray ordering use exact stored-coordinate predicates with spatial
witnesses. Positive Bezier hull certificates avoid endpoint-only tangent
searches. Geometry staging finishes before document edits, including failure
paths. Shared surface-cutting modes keep their existing admission and seam rules.

The [current validation record](../planar-networks-validation.json) covers
198 native workflows (195 distinct ordered source scenes). Both builds match
command status, source membership, complete source definitions, areas and
Undo/Redo. All 184 successful workflows also pass 3DM round trips, sampled
boundary comparison and normal-mesh generation, covering 222 output B-reps.
Application tests repeat the corpus without opening a window; prompt tests
cover picking, deletion and cancellation.

Boundary comparison samples every nonzero span at 17 stations in both
directions, with a `1e-6` model gate. It is not a continuous error certificate,
native encoding or triangulation parity. The former
`chains_filled_outside_tangent` gap is closed; its measured native area 116 and
boundary now match without an exception. The earlier
[validation record](../planar-chains-validation.json) retains the historical
decline. Comparative speed and broader branch configurations remain
unqualified.

```sh
cargo test -p viboceros-oracle --test planar_chains -- --nocapture
cargo test -p viboceros-command --test planar_chains
cargo test -p viboceros planar_chains
python3 -m unittest tools.rhino_oracle.test_planar_chains
```

Repeat Rust checks with `--features native-smlib` for the usd-brep adapter.
Native captures use the private Xvfb launcher; no desktop windows are needed.
