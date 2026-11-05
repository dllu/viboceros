# Flip

`MatchCrvDir` uses the first selected curve as a reference and aligns the
remaining selected curves with it. `MatchCrvDir Reference=name-or-id` uses an
existing reference curve outside the target selection. It keeps each object's
identity, attributes, and geometry type; only curves needing reversal change.
The direction predicate is checked against 13 live RhinoCommon cases, including
open curves, closed circles with different seams, arcs, and polylines.

`Flip` (aliases `Reverse`, `Rev`) reverses selected curve directions, mesh face
winding, open SubD winding, and surface/B-rep face orientations. Invoke it on a
selection or enter the command first and pick objects, then press Enter.

Closed SubDs and consistently oriented B-rep solids, including single-face
spheres, are left unchanged. Closed B-reps with inconsistent face orientations can flip;
this reverses their faces without repairing the inconsistency. Closed meshes
can point inward and are flipped normally. Unsupported objects such as points
are skipped instead of failing the whole mixed selection. Preselection remains
selected. Command-first picking releases flipped objects and all SubDs, including
skipped closed SubDs. Other skipped group members stay selected without
re-expanding the group.

Surface orientation is not UV reversal. Knots, coefficients, weights, parameter
domains, trims, and edges remain unchanged. An untrimmed NURBS surface is promoted
to a natural single-face B-rep to store its independent face sense. Undo restores
the original representation. Existing B-reps only toggle their face-sense flags.
All replacements are staged before mutation, preserve identity and metadata, and
form one undo step. An all-skipped selection leaves undo and redo intact.

[`Dir`](direction.md) shares the normal-flip policy and also edits surface/B-rep
U/V directions while preserving oriented normals and trims. It supports explicit
polysurface face batches, command-first picking and native selection replay.
Its live inspector supports SubD normal arrows and provisional reversal; see
[SubD direction evidence](subd-direction.md). Clipping planes, lights and
subobject direction picking remain unimplemented.

## Evidence and limits

The [orientation audit](../orientation-audit.md) retains 36 public Rhino cases,
including actual named `Flip` command events for 16 pre/postselection cases.
Its follow-up adds 20 compound-solid cases and 16 clean closed-but-unoriented
cases; the latter distinguishes solid rejection from mere topological closure.
Complete before/after definitions distinguish face-sense changes from UV edits.
Native command and application tests exercise the same behavior with independent
geometry fixtures, actual selection workflows, and undo/redo. These are behavioral
witnesses, not an identical-source native replay of Rhino's primitive factories.

Document admission normalizes known inward single-shell solids, including SubDs.
The kernel independently represents inward shells and preserves signed volume.

Public reference: [Rhino 8 Flip](https://docs.mcneel.com/rhino/8/help/en-us/commands/flip.htm).
