# ToNURBS mesh and SubD edges

Ctrl+Shift-click edges before `ToNURBS`, or select them during its object prompt.
Ctrl+Shift-double-click selects an [edge loop or bounded range](component-edge-loops.md).
[SelEdgeRing](component-edge-rings.md) selects rings before conversion.
Press Enter to accept prompted input, set `DeleteInputObjects`, then Enter to
convert. Whole objects and edge components can share one invocation. Edge-only
input offers no MeshOptions or SubDOptions submenu. Escape clears component
picks without editing geometry, accepting preferences, or consuming redo.

Each edge becomes a separate unselected NURBS curve with its source's name,
layer, color policy and ordered group memberships. Edge owners remain intact
with either deletion choice. Whole peers retain their ordinary replacement/copy
rules. Preselection follows source document order and then descending component
order within each owner; prompted input retains supplied action order. Conversion is one
undoable edit; component picks remain cleared through undo/redo.

Mesh edges and flat SubD edges produce degree-one control-net lines, with domain
`[0, length]`. Smooth SubD edges produce clamped cubic approximations of the
limit edge. Geometry uses the SubD display state when input is accepted, before
editing conversion options. A nested `SubDDisplayToggle` changes appearance
without changing the pending curves; appearance persists through undo/redo.

Headless callers can select one mesh/SubD owner and use:

```text
ToNURBS Edges=3,0 DeleteInputObjects=No
ToNURBS Edges=All Object=<uuid>
```

`Edge` aliases `Edges`; indices are zero-based mesh topology edges or active
SubD edge ordinals. Explicit `Object` does not require whole-object selection.
Duplicate edges convert once. Rust callers can mix `ToNurbsTarget::Whole` and
`Edge` through `execute_to_nurbs_targets`; `ToNurbsContext` also carries the
input display state for deferred confirmation. Whole selection takes precedence
over edges of the same owner. Invalid/unavailable targets reject the entire
invocation. A maximum of 100,000 targets and the shared 1,048,576 output-control
budget bound conversion; these are resource limits, not fitting tolerances.

Three immutable Rhino command captures cover 26 preselection workflows and 311
output curves: mesh, open grids, closed boxes, n-gon and extraordinary vertices,
soft creases with constant/variable chains, both display modes, mixed whole
peers, deletion, cancellation, nested display changes, undo and redo. Smooth
outputs exactly match the captured public `SubDEdge.ToNurbsCurve(true)` results.
Rust replays bound complete positive curve images at `1e-11`, preserving degree
and domain; the maximum observed bound is `1.18e-13`. Requests and producer
sources have verified content hashes. Native capture ran only in private Xvfb.

In-memory UI tests cover prompted picking, removal, windows, mixed action order,
clearing selection, cancellation, stale inputs and nested display changes.
Native command-first mouse picking is not qualified here. Loop selection has
separate [gesture and topology qualification](component-edge-loops.md). Smooth curves
reuse the finite SubD edge fitter, without a continuous certificate against the
mathematical limit. It refines the full source net even for a subset, so large
inputs can reach subdivision budgets; general performance parity is unqualified.

See [whole-object ToNURBS](to-nurbs.md) and
[SubD conversion](to-nurbs-subd.md).
