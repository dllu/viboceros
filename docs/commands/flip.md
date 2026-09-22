# Flip

`Flip` (aliases `Reverse`, `Rev`) reverses selected curve directions, mesh face
winding, and the face orientation of open surfaces and B-reps. Invoke it on a
selection or enter the command first and pick objects, then press Enter.

Closed B-reps, including single-face spheres, are left unchanged. Closed meshes
can point inward and are flipped normally. Unsupported objects such as points
are skipped instead of failing the whole mixed selection. Preselection remains
selected; after command-first picking, only flipped objects are deselected.
Skipped group members stay selected, without re-expanding the group.

Surface orientation is not UV reversal. Knots, coefficients, weights, parameter
domains, trims, and edges remain unchanged. An untrimmed NURBS surface is promoted
to a natural single-face B-rep to store its independent face sense. Undo restores
the original representation. Existing B-reps only toggle their face-sense flags.
All replacements are staged before mutation, preserve identity and metadata, and
form one undo step. An all-skipped selection leaves undo and redo intact.

`Dir Flip` currently supports curves and meshes only. `Dir UReverse`, `VReverse`,
and `SwapUV` are distinct edits to untrimmed surface parameterization; this work
does not claim parity with Rhino's interactive `Dir` surface menu, SubD, clipping
planes, lights, or subobject picking.

## Evidence and limits

The [orientation audit](../orientation-audit.md) retains 36 public Rhino cases,
including actual named `Flip` command events for 16 pre/postselection cases.
Complete before/after definitions distinguish face-sense changes from UV edits.
Native command and application tests exercise the same behavior with independent
geometry fixtures, actual selection workflows, and undo/redo. These are behavioral
witnesses, not an identical-source native replay of Rhino's primitive factories.

This command policy does not normalize every solid entering a native document.
The kernel still represents inward shells and preserves signed volume. Rhino's
single-shell insertion behavior is a separate compatibility gap under audit.

Public reference: [Rhino 8 Flip](https://docs.mcneel.com/rhino/8/help/en-us/commands/flip.htm).
