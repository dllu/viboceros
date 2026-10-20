# OrientCrvToEdge

Place an open curve on a surface boundary. Start `OrientCrvToEdge`, pick the
source curve near an end, select a target surface edge, then pick a destination
along that edge. The current implementation supports source endpoint tangents
parallel to the construction plane, and arbitrary source orientations when the retained
endpoint already lies on the edge. Other off-plane endpoint tangents are
rejected without changing the document; their roll is still under investigation.
The [early spatial pilot](../orient-curve-edge-spatial-pilot.json.gz) retains
the diagnostic inputs and outputs separately from the qualified fixtures.

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

There are 49 owned Rhino 8.32 command captures: planar and spatial profiles with
in-plane endpoint tangents, all four boundaries, rotated CPlanes, tilted and
warped faces, reversed face orientation, retained on-edge orientation, grouped
copies, cancellation, and real Undo/Redo. API and UI replays compare full NURBS
control points, weights, knots, object order, selection, and group membership.
See the [input fixtures](../../tools/rhino_oracle/fixtures/orient_curve_edge.json)
and [bare-toggle fixtures](../../tools/rhino_oracle/fixtures/orient_curve_edge_options.json).

Reproduce the captures and checks with:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_orient_curve_edge
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_orient_curve_edge --options
cargo test -p viboceros-command --test orient_curve_edge --test orient_curve_edge_native
cargo test -p viboceros --bin viboceros app::tests::orient_edge
python3 -m unittest tools.rhino_oracle.test_orient_curve_edge
```

The intended Rhino workflow is described in the
[official command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/orientcrvtoedge.htm).
