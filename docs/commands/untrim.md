# Untrim

Run `Untrim`, then click a surface or polysurface edge. Each accepted pick edits
immediately. `Undo` at the prompt reverses the last pick; Enter or Esc finishes
and preserves accepted edits in one external Undo record. Component preselection
is cleared and ignored.

- `AllSimilar=No` restores a connected exterior trim run or one interior hole.
- `AllSimilar=Yes` restores the complete exterior boundary or all holes on the
  picked face. Picking a natural exterior edge restores the complete boundary.
- `KeepTrimObjects=Yes` retains original boundary curves and detached joined hole
  walls. A partial exterior edit retains its entire original boundary curve.

Options persist independently of UntrimAll, UntrimBorder and UntrimHoles. New trim
objects use current-layer defaults; the source retains its identity, attributes
and groups. Invalid indices and stale sources fail before document changes.

The explicit command API is
`Untrim object-id edge-index [AllSimilar=Yes|No] [KeepTrimObjects=Yes|No]`.
Rust callers can prepare `UntrimSelection` and commit it atomically or into a
command history group.

## Coverage

Interior picks support joined hole walls. Exterior picks support standalone
faces, including exact partial restoration between natural UV boundaries.
Four-edge rectangular UV loops reproduce native spatial edge indices for all 24
source edge orders. Saved
cases also cover split source edges, shifted UV domains, reversed face sense,
and polynomial and rational surfaces. The allocation policy remaps references
after exact geometry construction. Replay compares every edge record and numeric
component index directly, with no normalization.

Other initial edge layouts, additional hole-edge allocation classes, and reversed
spatial proxies have unverified numeric component ordering. Loops outside the
rectangular allocation policy retain source edge order.

Joined trimmed exterior restoration, partial paths across seams or singularities,
and multi-edge rectangle picks remain unsupported. Unsupported edits leave the
source unchanged. Complete standalone restoration supports natural seams and
singular trims through the validated natural-face constructor.

## Oracle

Source recipes and raw Rhino 8 observations are in
`tools/rhino_oracle/{fixtures,observations}/untrim_components.json` and
`untrim_partial.json`, `untrim_upper.json`, `untrim_history.json`,
`untrim_ordering.json`, and `untrim_curved_partial.json`. The 213 cases include
96 rectangular source permutations, four split-edge cases, eight curved-surface
partial edits, and the original 105 command cases. They retain complete control
nets, scalar intervals, topology, metadata, per-click geometry and external Undo/Redo. Native geometry is
never used as an input recipe.

Live captures use private Xvfb and at most eight cases per owned Rhino process:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.untrim_component_capture tools/rhino_oracle/fixtures/untrim_components.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_components.json tools/rhino_oracle/observations/untrim_components.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_history.json tools/rhino_oracle/observations/untrim_history.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_ordering.json tools/rhino_oracle/observations/untrim_ordering.json
```
