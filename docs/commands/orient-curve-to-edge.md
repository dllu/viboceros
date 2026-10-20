# OrientCrvToEdge

Place an open curve on a surface boundary. Start `OrientCrvToEdge`, pick the
source curve near an end, select a target surface edge, then pick a destination
along that edge. Source endpoint tangents may point in any spatial direction.
When the retained endpoint already lies on the target edge, placement preserves
its orientation relative to that edge and surface.

For an off-edge source, the negative CPlane Z direction is projected perpendicular to
the tangent at the picked end, then perpendicular to the tangent at the opposite
end used as the anchor. A picked-end tangent near the CPlane normal
retains the CPlane up direction. When the anchor tangent is nearly parallel to
that seed, placement uses the picked-end frame, or the CPlane axes if both ends
are parallel. The reference command uses an
inclusive absolute unit-direction dot cutoff of 0.99 at both stages, independent
of document angle tolerance. The comparison follows public OpenNURBS derivative
normalization at the exact cutoff; the geometric frames use stable orthogonal
construction. Endpoint reference directions are cached with the immutable
source capture for cursor previews.

`Copy=No` moves the original and finishes. `Copy=Yes` permits repeated copies;
Enter or Escape finishes, retaining accepted copies in one Undo step. Copies
inherit the source attributes and join its existing groups. Copy follows
`RememberCopyOptions`, initially No. Typed destinations are constrained to the
selected edge, and previews do not change document objects or Redo history.

Complete scripted commands use one selected source, or an explicit source ID:

```text
OrientCrvToEdge 6,-2,0 Edge=<surface-id>,0 PickPoint=0,-6,0 Copy=Yes
OrientCrvToEdge 6,-2,0 Edge=<surface-id>,0 Face=0 PickedSources=<curve-id> PickPoint=0,-6,0
```

`PickPoint` identifies the source selection location. Rhino 8.32's public
command captures retain the endpoint opposite that located pick; Viboceros
matches this convention. Omitting `PickPoint` retains the curve start.
`Face=index` chooses the adjacent face on a shared edge; without it, shared
edges require an explicit choice. In the UI, enter `Face=index` before picking
the edge. The face must actually use that edge.

`FlipSurface` and `ReverseCurve` accept toggles or Yes/No values in the UI.
The reference build's captured assignments and bare toggles update its prompts
while leaving these placements unchanged; this implementation preserves that
observed behavior. This is a bounded compatibility result, rather than a claim
that all Rhino versions or source configurations treat those options alike.
Closed source curves, seam edges with multiple trims on the chosen face, and
temporary `SubCrv` sources are not supported yet.

The surface parent stays immutable throughout placement and history replay.
If the source changes or becomes unavailable, the command finishes. A changed
target discards the preview and asks for the edge again. Accepted copies remain.

There are 189 owned Rhino 8.32 command captures: planar and spatial profiles with
arbitrary endpoint tangents, all four boundaries, rotated CPlanes, tilted and
warped faces, reversed face orientation, retained on-edge orientation, grouped
copies, cancellation, and real Undo/Redo. Spatial cases include changed curve
shapes with the same endpoint tangents, interior source picks, parallel and
antiparallel ends, and both sides of the 0.99 cutoff. API and UI replays compare full NURBS
control points, weights, knots, object order, selection, and group membership.
See the [input fixtures](../../tools/rhino_oracle/fixtures/orient_curve_edge.json)
and [bare-toggle fixtures](../../tools/rhino_oracle/fixtures/orient_curve_edge_options.json),
plus the [spatial](../../tools/rhino_oracle/fixtures/orient_curve_edge_spatial.json),
[parallel](../../tools/rhino_oracle/fixtures/orient_curve_edge_parallel.json),
[tolerance](../../tools/rhino_oracle/fixtures/orient_curve_edge_threshold.json),
[cutoff](../../tools/rhino_oracle/fixtures/orient_curve_edge_cutoff.json), and
[boundary](../../tools/rhino_oracle/fixtures/orient_curve_edge_boundary.json) cases.
[Angle tolerance cases](../../tools/rhino_oracle/fixtures/orient_curve_edge_angle.json)
exercise both sides of the cutoff from 1e-7 to 0.5 radians.
The [early spatial pilot](../orient-curve-edge-spatial-pilot.json.gz) is retained
as a diagnostic from before the source-frame rule was resolved.

Reproduce the captures and checks with:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_orient_curve_edge
for mode in options spatial parallel threshold cutoff boundary angle; do
  tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_orient_curve_edge --"$mode"
done
cargo test -p viboceros-command --test orient_curve_edge --test orient_curve_edge_native
cargo test -p viboceros --bin viboceros app::tests::orient_edge
python3 -m unittest tools.rhino_oracle.test_orient_curve_edge
```

The intended Rhino workflow is described in the
[official command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/orientcrvtoedge.htm).
