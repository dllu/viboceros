# Conic centers: API and point-prompt audit

[Ellipse recognition](elliptic-center-snaps.md) · [Original circular audit](circular-center-snaps.md) · [Provenance](conic-center-provenance.json)

Two fresh owned-private-Xvfb runs use Rhino 8.32.26160.13001. The
`nurbs_curve_conic_centers` protocol operation calls public
[TryGetCircle](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/trygetcircle)
and [TryGetEllipse](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/trygetellipse)
with explicit absolute tolerance `1e-9`, retaining all three center coordinates.
It does not normalize native parameter domains or insert geometry into a document.
This complements actual point-prompt snapping; API recognition is not evidence
that Rhino's interactive snap policy makes the same choice.

The independent [generator](../tools/rhino_oracle/references/conic_centers.py)
constructs curves without reading either engine's output. Families cover full and
partial ellipses, reversal, degree 5/12 elevation, refinement, extreme and shifted
domains, signed/common gauges, unequal endpoint weights, stationary quartic
parameterization, rotation/shear, off-plane and tilted curves, eccentricity,
short arcs, nonconics and a circle control. Far translation is API-only.

| Evidence | Matches | Explicit remaining differences |
| --- | ---: | --- |
| [32 API records](../tools/rhino_oracle/observations/conic_centers.json) | 29, absolute-only `1e-9` | One far-origin rejection in Rhino; two short-arc center drifts |
| [62 point-prompt records](../tools/rhino_oracle/observations/elliptic_center_snaps.json) | 42 complete command/history replays | Six gauge-related admission differences; four short-arc target differences |

Ten further point-prompt cases agree on nonconic admission misses. They are
admission-only comparisons, not full unsnapped-point geometry matches. All 62
actual SplitEdge commands succeeded and performed Undo/Redo. Each curve family
uses both one-shot Cen over five persistent modes and persistent Cen alone.
Public prompt-time matrices agree with `WorldToClient` within `1e-7` pixels;
native queries use actual integer clicks and the same 12-pixel aperture. Native
computed targets, never recorded Rhino result points, drive command replay.
Complete ordered geometry, attributes, selection and history use absolute
epsilon `1e-9`, relative `1e-10`. These constrained results do not establish
unconstrained GetPoint, depth/occlusion ordering or unrestricted priority parity.

## Corrections prompted by the audit

Thin ellipse recognition now uses independent scales along empirical planar
principal directions. A second small SVD recovers physical ellipse axes after
undoing that affine map. This avoids treating physical eccentricity itself as an
unstable conic fit. Native tests also cover tilted degree-5 ellipses with axis
ratios through 2,000; those additional ratios are analytic tests, not extra Rhino
observations.

Rational quadratics additionally have an exact binary-input center proposal.
For `q = w1²/(w0*w2)`, positive common-sign weights and `0 < q < 1`,
`C = ((P0+P2)/2 - q P1)/(1-q)`. Exact rational arithmetic delays coordinate
rounding until the final center. The control triangle also supplies two possibly
oblique ellipse axes, converted to physical principal axes with SVD. The widest
eligible span proposes the center; every original span must still pass the
existing plane/radial certificate. This is linear in the number of spans, not
repeated full-curve testing of every candidate.

An exact quadratic center describes the supplied binary64 control geometry. It
does not recover an unknown pre-rounding design center after ill-conditioned
subdivision. General higher-degree fits retain their conditioning guard and can
remain inconclusive. Whole-span recognition is still a conservative floating-point
test, not an exact algebraic acceptance predicate.

Circular recognition now requires regular curvature jets to agree on center and
radius. Jets use local model coordinates and normalized parameter domains. A
short ellipse can lie entirely within a circle's absolute tolerance tube while
having a changing osculating center; that is no longer enough to classify it as
circular. Quadratics also require agreement with their algebraic conic center
and both physical semi-axes, covering still shorter arcs whose sampled curvature
change falls below tolerance. The whole-span coefficient check remains mandatory,
including the adversarial bump invisible to sampled second-order jets.

## Disagreements retained without normalization

Rhino's API recognizes ellipses under all tested common weight gauges, but its
interactive Center snap misses the positive ×8, negative ×−8 and positive ×1e200
versions. Native snapping retains the geometry-invariant center. The ×1e−200
ellipse captures in both engines.

For the shortest arc, Rhino's API reports `x≈4.0057587513`, while its point command
splits at `x≈2.5000013132`. Native exact quadratic recognition gives
`x≈4.0000000002`; its full center remains within `1e-9` of the generator's design
center. The preceding short-arc family differs by about `5.4e-7` in the API and
`3.1e-7` in snapping. We preserve these records as differences, not approximate
matches. Rhino's approximate-conic snap policy remains unfinished natively.
Rhino also rejects the ellipse translated by `1e12`, while the native center
retains the exact translation. Absolute-only API comparison prevents that origin
from widening the accepted center error.

The original 44-case circular audit remains unchanged and still has 38 full
matches, four admission-only misses and two negative-gauge differences. API
timings include different language bridges and work boundaries; no kernel
speedup, viewport FPS or frame budget is claimed here.

Verification checkpoint: 3,080 release-mode workspace tests passed (25 ordinary
opt-in exclusions), 290 Python tests, seven explicitly run offscreen GPU tests,
formatting, and Clippy/Rustdoc with warnings denied. A repeated circular-NURBS
hover timing check measured `0.787`, `0.669` and `1.190` ms per warm query for
1, 100 and 1,000 separated sources, respectively (100 queries each). This is
consistent with the previous local circular audit and is not a scene-wide budget
or a comparison with Rhino.

```sh
python3 -m tools.rhino_oracle.references.conic_centers api
python3 -m tools.rhino_oracle.references.conic_centers snaps
cargo test --release -p viboceros-oracle full_coordinate_conic_api
cargo test --release -p viboceros-oracle elliptic_snaps_replay
python3 -m unittest tools.rhino_oracle.test_curve_conic_centers
```
