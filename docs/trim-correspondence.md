# Trim correspondence with stationary spans

[Solid orientation](solid-orientation.md) · [B-rep meshing](brep-meshing.md)

The image of a valid rational UV trim need not move at every parameter. A
quadratic with controls `[a,a,b,b,b]`, weights `[1,2,1,2,1]`, and knots
`[-3,-3,-3,2,2,7,7,7]` traverses the segment `a→b` on its first span and remains
at `b` on its second. Clamping, continuity, same-sign weights, and endpoint-copy
controls prove the segment image independently of numerical correspondence.

## Search and safeguards

The previous composed trim search retained only the sixteen nearest sample
starts. Samples on a stationary span could occupy every slot and exclude all
starts on the active span. Valid boxes, tetrahedra, and spheres then failed
construction with `a model-space edge interior leaves its lifted p-curve`.

`brep/trim_image::distance_witness` orders all supplied starts by distance without
discarding the more distant ones. A near evaluated point permits early return;
otherwise every start remains eligible for refinement. Each start is constrained
to its original nonempty knot span. At its upper endpoint, the curve's left jet
is used. This prevents an improving Newton step from landing on a neighboring
constant piece and stopping with zero tangent. Samples retain their side of a
knot, so a stored left-limit position is not refined using a right-limit jet.

The query is a numerical distance-witness search, **not a certified global
minimum**. It retains 64 refinement iterations and 24 backtracks per iteration;
total work scales with the supplied finite seed set. Validation tolerances and
the bidirectional edge/trim checks are unchanged. Unsupported local minima,
unsampled geometry, and floating-point evaluation remain limitations of this
sampled validator; removing seed starvation is not a proof of global B-rep validity.

Edge splitting and conforming meshing use the same query. Split correspondence
now charges its degree-weighted refinement allowance for every supplied start,
rather than assuming sixteen. It retains the existing global work limit and
checks that the found trim parameters are strictly monotone. Meshing still
checks the final lifted boundary positions against their shared model-space edge.

Independent trim sampling removes exact consecutive UV duplicates, including a
duplicate closing point left by a stationary last span. Zero-length sampled
polygon edges then do not obstruct triangulation. Nearby distinct coordinates
and nonconsecutive repeated contacts are retained; neither the original curves
nor their knot vectors are modified. Tests preserve even a minimum-subnormal UV
separation and an explicit retraced segment.

Native regressions cover both senses of boxes, tetrahedra, and spheres with
degrees 2/3/7, leading/trailing stationary spans, and positive/common-negative
weight gauges. A single active seed behind 65 stationary seeds has an analytic
solution: `x=s²/(1+2s−2s²)` on the active span. Additional cases reject off-edge
bulges and reverse-direction edge excursions, and exercise splitting and closed
meshing with four stationary spans per trim. These extra cases are native tests,
not additional Rhino observations.

## Same-source Rhino evidence

The [source-only generator](../tools/rhino_oracle/references/stationary_trims.py)
creates an [18-case request](../tools/rhino_oracle/fixtures/stationary_trims.json):
boxes, tetrahedra, and spheres, each with regular, leading-stationary, and
trailing-stationary quadratic trims, in both global face senses. All capture
weights are positive. The bounded `trim_endpoint_encoding` source recipe uses
copies of the original UV endpoints; it does not interpolate nearly collinear
controls, simplify curves, or bypass constructor validation.

Native geometry is exported to owned 3dm artifacts and its round trip checked
before Rhino reads that identical source. Rhino 8.32.26160.13001 ran on a separate
owned private Xvfb/i3 display. It accepted all 18 B-reps as valid. The
[unaltered records](../tools/rhino_oracle/observations/stationary_trims.json)
retain every definition, knot, topology field, sense, and tolerance. No document
insertion or normalization is involved.

The [baseline native outcomes](../tools/rhino_oracle/observations/stationary_trims_native_before.json)
retain six constructor errors, not `Unknown` orientations or skipped cases.
The [before report](../tools/rhino_oracle/observations/stationary_trims_before_report.json)
has 12 matches and six native failures. The
[after report](../tools/rhino_oracle/observations/stationary_trims_after_report.json)
has 18 matches, zero failures, and zero numeric differences in all complete
records. The observations and their original IEEE-754 values, including signed
zero, were preserved during compaction. Query timing fields are zero; these
captures make no speed claim. [Provenance](trim-correspondence-provenance.json)
records the artifacts, source hashes, and verification scope.

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/stationary_trims.json \
  --observations tools/rhino_oracle/observations/stationary_trims.json \
  --absolute-epsilon 0 --relative-epsilon 1e-12
cargo test -p viboceros-geometry --release stationary
```
