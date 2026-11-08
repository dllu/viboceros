# MergeSrf

[Surfaces](surfaces.md) · [NURBS support merging](../smlib-surface-merging.md)

`MergeSrf` merges two untrimmed NURBS surfaces sharing one complete boundary.
It works with polynomial and rational supports in either parameter direction,
including opposite traversal, transposed UV axes and unequal degrees.

Preselect two surfaces to merge immediately. Otherwise enter `MergeSrf`, edit
`Smooth`, `Tolerance` or `Roundness`, and pick two surfaces. The second pick
executes. `Roundness` accepts values from 0 to 1, including fractions;
`Tolerance` must be at least the document absolute tolerance. Bare `Tolerance`
and `Roundness` open numeric getters; Enter keeps their values. Esc cancels.
Option changes survive cancellation and failed merges. The remembered joining
tolerance increases when required by a document's absolute tolerance.

The result replaces the first surface in place, retaining its identity,
attributes, groups and face orientation; the second surface is deleted.
Preselection uses document order; command-first selection uses pick order.
Preselected results stay selected. Command-first picks are released after
success, failure or cancellation. Undo/Redo restore the corresponding model
state, and `SelLast` recalls the replaced surface.

For selected inputs the complete command API accepts:

```text
MergeSrf Smooth=Yes Tolerance=0.0001 Roundness=0.5
```

The portable build uses exact NURBS basis conversion and homogeneous seam
averaging; the native build uses owned SMLib operands. Smooth moves the seam
knots and changes the surface image. Joining tolerance bounds boundary admission,
not the shape change caused by smoothing.

Trimmed surfaces, ambiguous shared boundaries, nonlinear boundary-speed matching
and broader closed/periodic inputs remain unsupported or unqualified. The
[finite command validation](../merge-srf-command-validation.json) covers retained
headless Rhino captures, API/UI replay, topology, metadata, history and 3DM
round trips. Surface comparisons use a parameter grid; they do not establish a
continuous surface-distance bound or full Rhino parity.

```sh
python3 -m tools.rhino_oracle.check_merge_surface_command
python3 -m unittest tools.rhino_oracle.test_merge_surface_command_record
```
