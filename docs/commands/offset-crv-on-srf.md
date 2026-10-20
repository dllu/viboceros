# OffsetCrvOnSrf

Create surface-distance offsets from one curve or surface edge. Preselect a
curve, or start `OffsetCrvOnSrf` and pick a curve or edge. Pick a base surface
or B-rep face on the desired side, then enter a signed distance. `Flip` or a
viewport click reverses the side. `Number` sets the number of sequential
offsets; each copy advances the previous complete offset before face trimming.
This avoids duplicate copies when a hole splits only one of the offsets.

The viewport displays cached curves without editing the document. Enter
accepts the current distance. Escape discards the preview and preserves Redo.
Accepted curves use fresh attributes on the current layer; original selection
is retained and outputs remain unselected, matching the captured public
command. One Undo/Redo replays all copies. Distance and Number are remembered
for subsequent interactive invocations; Flip starts afresh.

`ThroughPoint` changes the distance prompt into a repeated point getter.
Each accepted point offsets the original source once, ignoring Number and Flip.
Typed points and viewport picks are constrained to the selected face, including
its outer trims and holes. Enter finishes; Escape retains accepted curves.
All accepted points share one Undo/Redo entry. A failed point preserves earlier
curves and leaves the getter active. Hover previews remain outside document
history, and committing a cached preview does not repeat the inverse solve.

At the first ThroughPoint prompt, `Select` accepts existing point objects.
Click points, use a selection window, or type their object IDs in the desired
order. Enter creates one offset per selected point and finishes the command.
On-source points are skipped; Number and Flip are ignored. The ordered batch
is fitted before any output is added and shares one Undo/Redo entry. Selection
is cleared when entering this getter. Cancel and an empty selection create no
geometry and preserve Redo. Other unresolved geometry fails without committing
part of the batch. Selecting more than 1024 point objects is unsupported.

At source selection, `SubCrv` picks a temporary directed portion: start and end
points, or a curve length with direction confirmation. `Direction=Locked|Free`
uses the shared directed-subcurve solver. Escape returns to source selection.
The source and any edge parent remain unchanged. Changes to a captured source,
face or model tolerance invalidate the getter.

Scripted forms use native curve parameters for SubCrv:

```text
OffsetCrvOnSrf 1 Source=<curve-id> Surface=<surface-id> Number=3
OffsetCrvOnSrf -0.5 Edge=<brep-id>,0 Surface=<brep-id> Face=1
OffsetCrvOnSrf 1 Source=<curve-id> Surface=<surface-id> SidePoint=0,3,0 Flip=Yes
OffsetCrvOnSrf 1 Source=<curve-id> Surface=<surface-id> SubCrv=6,2
OffsetCrvOnSrf ThroughPoint=0,2,0 Source=<curve-id> Surface=<surface-id>
OffsetCrvOnSrf ThroughPoint Points=<point-id>,<point-id> Source=<curve-id> Surface=<surface-id>
```

Without `SidePoint`, positive distance follows the left side of the curve in
the face orientation. Explicit picks choose the physical side independently
of face reversal. Zero distance and Number outside 1–1024 are rejected.
Geometry is prepared before any output is added; failed operations preserve
the model, selection and history.

Planar open offsets extend along endpoint tangents to the natural surface
boundary, then clip against face loops and holes. This command policy differs
from the public SDK's source-endpoint-preserving producer. It reuses existing
curve boundary extension and certified face clipping. Closed offsets use the
qualified affine-planar gap reconnection policy where clipping is necessary.
Curved open inputs extend along their endpoint normal sections before offsetting
and extend retained open results to the natural rectangle. A normal section
lies in the plane spanned by the curve tangent and surface normal. Existing
surface/plane intersection producers supply the sections; unresolved or
unsupported cases fail without model edits. Complete surface-image bounds
check each extension and the restored native knot times.

Build with `--features native-smlib` to continue endpoints on general NURBS
patches. The existing usd-brep surface intersector supplies candidates after
the portable analytic dispatcher declines. Arc-length fitting removes trace
parameter scaffolding; complete surface-image bounds and exact control-hull
plane bands verify each retained section. Endpoint tangents follow the normal
plane's intersection with the surface tangent plane. Incomplete traces fail
before model edits. Endpoints already within the physical numerical budget
use an exact distance check instead of requesting a tiny intersection.

Four further native command scenes exercise a higher-degree patch, reversal,
a straight isocurve and a rational patch. All produce geometry with source
immutability and one Undo/Redo entry. The diagonal and rational cases meet
the unchanged `1e-6` gate; their measured differences are about `8.7e-7` and
`5.2e-7`. The reversed case remains unqualified at `1.04e-6`, and the isocurve
at `2.15e-5`. Independent graph-metric integration and a public SDK
closest-point query confirm the latter discrepancy. Rational-patch viewport
previews use the same producer and cached confirmation.

On affine planar patches, tangent-discontinuous sources use the shared planar
offset producer. Inside corners trim to their intersection; outside corners
use a polynomial quadratic between the offset endpoints and tangent apex,
as in the captured public SDK. These spans are parabolic conics. Their
midpoints differ from circular fillets even for a right-angle source.
Open and closed polylines, alternating turns, reversal, a tilted plane and a
mixed line/Bézier source are covered by nine SDK captures. Their maximum
sampled difference is `2.2e-7`. Open straight outputs use chord-length parameters
after endpoint extension; closed inward polygons retain the durations of
surviving source intervals. Natural-boundary clipping retains complete
surface-image checks, including an offset split into four surviving corners.
Eight command/getter scenes also meet `1e-6`, including their native
linear parameter domains, metadata and history. The mixed curve command
remains unqualified at `3.18e-5`. An exact Bernstein point at `s=5/8` lies on
Viboceros's bridge; a separate public SDK closest-point query confirms that
the captured command bridge misses it by the same amount. This diagnostic
ceiling does not change the qualification gate. Curved-surface corner joins
and general collapsed/self-intersecting cases remain unresolved.

Periodic pullbacks retain independent UV limits at a common spatial seam.
The command emits separate chart portions and advances one complete curve
for each Number copy. Sources share preparation across point picks and option
changes; confirming an unchanged distance or Number reuses the current preview.

Eight native scenes cover shortened cylinder generators and ring arcs, warped
isocurves and diagonal paths, including the cylinder diagonal's two output
portions. Five qualify against the unchanged `1e-6` sampled gate, including the warped
diagonal, whose gap fell from about `0.282` to less than `4e-7`. The cylinder
diagonal and the two warped isocurves remain unqualified.
In the diagonal capture, independent cylinder unrolling identifies a
`2.45e-3` discrepancy in the SDK offset's starting endpoint before the command
continues it. Independent [SDK captures](../../tools/rhino_oracle/observations/surface_curve_offset_extensions.json)
also compare the source extensions: seven of eight qualify; the native cylinder
diagonal continuation itself drifts about `2.9e-6` off the exact surface.
See the [current measurements](../surface-geodesic-qualification.json).

Eight fixed public Rhino command captures cover signed distance, Flip,
Number persistence, line/NURBS inputs, preselection and postselection,
planar tangent extension, trimmed holes and grouped Undo/Redo. Both scripted
and UI paths replay their full output geometry and final metadata. Seven
meet the unchanged `1e-6` sampled geometry gate. The Bézier case remains
unqualified at about `4.24e-6`; a separate diagnostic ceiling prevents regressions
without treating it as parity. The earlier SDK matrix retains its five
unqualified cases. See [geometry and qualification evidence](../surface-geodesic-offset.md).

The ThroughPoint solver finds a source parameter and signed distance using the
existing normal geodesic shots and thin QR solve. Parameter-cell control hulls
skip source regions whose Euclidean chord already exceeds an accepted surface
distance. Finite work limits, line search, regular-target checks and a final
closest-point check reject unresolved points. Multiple equal normal branches
are ambiguous. This bounded numerical search does not certify the global
cut locus or all roots of an arbitrary rational source.

Sixteen further native command captures cover repeated and failed picks,
Number being ignored, off-surface projection, sphere and cylinder charts,
temporary source-end extension, cancellation, and hole-boundary ties under
rotation, U/V reversal and edge splitting. Fourteen qualify within `1e-6`.
The captured Bézier and cylinder cases remain unqualified. Nine independent
[SDK queries](../../tools/rhino_oracle/fixtures/surface_offset_through_verify.json)
confirm that a native Bézier curve is `3.89e-3` from an analytic normal-family
point near a fold, and a native cylinder curve misses its requested point by
`3.49e-5`. These diagnostic ceilings do not change the parity gate.

Open affine-planar fits impose analytic endpoint derivatives through the
shared collocation solver's fixed control rows. This keeps subsequent tangent
extensions from amplifying position-fit derivative errors. The same constraints
work with the optional usd-brep interpolation producer.

Eight additional native captures qualify point-object selection order, off-surface
projection, on-source skips, repeated offsets, cancellation, empty selection,
attributes and Undo/Redo. The five output cases meet the `1e-6` sampled gate with a maximum
difference of about `2.2e-14`; the other three create no output. Captured point
objects, the source, and the target remain geometrically unchanged.

General closed-gap reconnection, curved-surface corner joins, cut-locus removal,
pole crossings and complete topology certification remain pending. These
limits preclude a claim of complete Rhino compatibility.

```sh
cargo test -p viboceros-command --lib surface_curve_offset
cargo test -p viboceros-command --test surface_curve_offset_native -- --nocapture
cargo test -p viboceros-command --features native-smlib --test surface_curve_offset_native
cargo test -p viboceros-command --features native-smlib --lib surface_curve_offset
cargo test -p viboceros --features native-smlib --bin viboceros surface_curve_offset
cargo test -p viboceros --bin viboceros surface_curve_offset
python3 -m unittest tools.rhino_oracle.test_surface_offset_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command --corners
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_curve_offset --corners
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_corner_verify
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command --through-point
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command --select-point
```
