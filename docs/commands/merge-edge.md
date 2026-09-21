# MergeEdge

[Command reference](README.md) · [MergeAllEdges](merge-edges.md) · [Certified kernel](../selected-edge-merge.md)

Enter `MergeEdge`, then click a surface or polysurface boundary edge. Object
preselection is cleared. Surface isocurves, mesh wires and construction-plane
points are not edge components. Overlapping hits produce numbered choices;
hover a choice to highlight that component, then click it or enter its number.
You can pick again without editing the document.

With two eligible neighbors, `EdgeA` joins the immediate start neighbor, `EdgeB`
the end neighbor, `Both` joins one neighbor at each end, and `All` recursively
merges the selected chain. A/B labels mark the selected curve's spatial ends.
With one eligible neighbor, the options are `Edge` and `All`. An unmergeable pick
returns to edge picking. Type an option or use its button; Enter, Escape and
Cancel leave the geometry unchanged. A successful choice ends the command.

Explicit scripts can use `MergeEdge object-id edge-index [Edge|EdgeA|EdgeB|Both|All]`.
Indices are zero-based and the default explicit choice is `All`. This native
component-address syntax is not Rhino's mouse macro syntax.

The angular tolerance is clamped to 0.1°–1°. Every merge uses the kernel's
whole-curve spatial certificate, exact UV certificate, valence-two/trim-incidence
checks and bounded work. Unlike `MergeAllEdges`, unrelated straight edges are
not simplified and kinky faces are not split. IDs, attributes, layers and groups
remain intact. One successful choice creates one Undo step; Undo and Redo do not
restore the transient selection. Cancellation/no-op prompts create no history
entry and leave an existing redo branch intact.

Preparation is read-only; no document transaction stays open during a prompt.
Source geometry and tolerance snapshots reject stale picks, including candidates
in an ambiguity menu. Hidden/locked objects cannot be edited. Computation or
replacement failures roll back atomically.

## Evidence and limits

Native command replay covers the earlier [15 mouse cases](../merge-edge-mouse-provenance.json)
and [21 additional cases](../merge-edge-command-provenance.json), with
[requests](../../tools/rhino_oracle/fixtures/merge_edge_command.json) and
[complete Rhino responses](../../tools/rhino_oracle/observations/merge_edge_command.json).
These include endpoint choices, no-ops, angular candidate limits, a kinked face,
preselection cleanup, cancellation and actual Undo/Redo in an owned Xvfb session.

Native edge-table ordering differs from Rhino's command ordering. Tests apply
only explicit recorded edge permutations and the resulting incidence-witness
serialization order, then compare all geometry/attribute/history fields at
absolute epsilon `1e-9`, relative epsilon `1e-10`. Ordered UV definitions are not
rotated, fitted or discarded. This establishes bounded fixture agreement, not
arbitrary-input or exact component-number parity.

Viewport capture uses the cached segments used to display curves, with an
eight-pixel aperture, not an analytic ray–NURBS intersection. All overlapping
captured edges remain available instead of silently choosing by depth. General
Rhino occlusion/menu behavior and adaptive high-zoom curve sampling remain open
work. This command does not imply completion of the broader CAD application.
