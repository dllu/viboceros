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
Curved open inputs continue their endpoint spans in normalized surface
parameters, then offset the continued curve and extend retained open results
to the natural rectangle. Complete surface-image bounds check each proposed
extension, including the restored native knot times. Smooth image fitting
prevents artificial tangent jumps between independent image segments.
Captured sources share this preparation across point picks and option changes;
confirming an unchanged distance or Number reuses the current preview.

Eight native scenes cover shortened cylinder generators and ring arcs, warped
isocurves and diagonal paths. The four cylinder generator/ring cases meet the
unchanged `1e-6` sampled gate, including UI previews and Undo/Redo. Warped
isocurves remain unqualified at about `2.63e-5`; the warped diagonal differs by
about `0.282`. The seam-crossing cylinder diagonal is rejected when its
pullback cannot be certified, preserving the document. These are unresolved
compatibility gaps. Independent [SDK captures](../../tools/rhino_oracle/observations/surface_curve_offset_extensions.json)
also record public `Curve.ExtendOnSurface` before and after `OffsetOnSurface`.

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

General closed-gap reconnection, corner joins, cut-locus removal,
pole crossings and complete topology certification remain pending. These
limits preclude a claim of complete Rhino compatibility.

```sh
cargo test -p viboceros-command --lib surface_curve_offset
cargo test -p viboceros-command --test surface_curve_offset_native -- --nocapture
cargo test -p viboceros-command --features native-smlib --test surface_curve_offset_native
cargo test -p viboceros --bin viboceros surface_curve_offset
python3 -m unittest tools.rhino_oracle.test_surface_offset_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command --through-point
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_surface_offset_command --select-point
```
