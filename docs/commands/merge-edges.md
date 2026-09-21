# MergeAllEdges

[Command reference](README.md) · [Certified kernel](../join-edge-cleanup.md)

`MergeAllEdges` coalesces redundant edges on selected NURBS surfaces and B-reps.
Select objects first, or enter the command, pick surfaces, and press Enter.
There are no options. Curves, meshes, and points are ignored in mixed
preselection; a selection containing only unsupported objects is released.
Escape cancels command-first picking without editing geometry.

The angular cutoff follows the document angular tolerance, clamped to
**0.1°–1°**. The absolute tolerance still bounds the accumulated spatial curve
change. Branch vertices, singular-trim vertices, incompatible trim incidence,
and uncertifiable merges are retained. This is not a general edge repair or
surface healing command; interactive `MergeEdge` is not implemented.

Objects retain their IDs, attributes, layers, and group memberships. All changed
objects form one atomic undo step. Preselection is retained through Undo/Redo;
command-first picks are released before recording geometry edits and are not
restored by Undo/Redo. Geometry or commit failures roll back the entire command.
Native no-ops preserve the original representation and redo branch, including
bare NURBS surfaces.

## Oracle evidence and limits

The public Rhino 8 command was exercised on shared native-generated 3DM sources,
with native round-trip verification and topology-preserving Rhino insertion.
The retained datasets contain 39 selection/history/smooth-split cases,
85 planar angular-cutoff cases, and 64 kinked-surface cases:

- [Command fixtures](../../tools/rhino_oracle/fixtures/merge_edges_command.json)
  and [raw observations](../../tools/rhino_oracle/observations/merge_edges_command.json).
- [Planar angle fixtures](../../tools/rhino_oracle/fixtures/merge_edges_angles.json)
  and [raw observations](../../tools/rhino_oracle/observations/merge_edges_angles.json).
- [Kinked-surface fixtures](../../tools/rhino_oracle/fixtures/merge_edges_kinky_surfaces.json)
  and [raw observations](../../tools/rhino_oracle/observations/merge_edges_kinky_surfaces.json).
- [Source/observation hashes](../merge-edges-provenance.json) and
  [full raw comparison](../merge-edges-comparison.json).

These are correctness probes, not performance measurements. Every Rhino run
uses an owned private Xvfb display. History probes defer until the setup Python
script has returned: nested Undo can report Success while saying “Nothing to
undo.” EndCommand captures the target command, and post-command snapshots verify
that Undo/Redo actually restore the expected topology. Successful driving macros
do not append Cancel, which would clear selection before testing Undo.

Full raw Rhino equivalence is **not** claimed:

- Rhino reparameterizes straight spatial edges and UV trims, and normalizes
  constant line weights and redundant collinear controls. Native cleanup retains
  untouched curves. The narrower
  regression comparison checks edge samples, topology, surface/trim controls,
  integrals, uncertainty, identity, attributes, groups, and selection; raw
  parameterization differences remain in the report.
  Unit-weight degree-one axis-aligned monotone UV control polygons are compared
  by their exact line locus rather than redundant interior control count.
- 3DM omits phantom outer surface knots; their reconstructed values can differ
  from the source's unused outer knots, already before the command runs.
- Rhino replaces some unchanged objects and creates undo records. Native no-ops
  deliberately do neither, preserving an existing redo branch.
- Rhino can split kinked surfaces into additional faces during replacement.
  Native edge cleanup does not split surfaces. Planar trim fixtures isolate the
  edge-angle policy from this separate behavior.
- At an exact floating-point angular cutoff, Rhino's cosine comparison and the
  native stable `atan2` comparison can disagree. Near-cutoff raw results are
  retained; the geometry kernel does not round genuine kinks away.

The full raw comparison has **84 matches out of 188**: 81/85 planar angle cases,
3/39 command cases, and 0/64 kinked-surface cases. Four planar differences are
at the exact requested 0.2°, 0.5°, or 1° cutoffs; the native evaluated angles
exceed the cutoff by less than `1e-14` radians. The 39-case narrower regression
also checks the original and merged geometry through Undo/Redo, with the four
documented no-op history differences kept explicit. Of the kinked-surface cases,
33 cause Rhino to split one face into two (seven edges instead of five).

An initial axis-aligned planar probe was rejected by the shared-artifact guard:
3DM reclassified a newly split trim from non-isoparametric to a boundary isocurve.
The final planar matrix uses oblique interior trims and passes the unchanged
round-trip guard; no classification difference is silently normalized.

The [kernel guarantees and limits](../join-edge-cleanup.md) also apply, including
exact UV preservation certificates, bounded work, and degree limits.
