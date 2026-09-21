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

Certified straight edges become degree-one, unit-weight curves parameterized
from zero to chord length. Exactly straight UV trims are simplified too; their
surface-space loci are unchanged. Approximate spatial simplification shares the
preceding merges' displacement budget and retains a conservative error bound.
Untouched edges are visited; this extra simplification is not applied by Join.

Objects retain their IDs, attributes, layers, and group memberships. All eligible
objects form one atomic undo step, even when no edges merge or geometry is
already clean. Bare NURBS surfaces become B-reps; Undo restores the original
representation. Replacement clears an existing redo branch, matching the measured
Rhino policy. Preselection is retained through Undo/Redo;
command-first picks are released before recording geometry edits and are not
restored by Undo/Redo. Geometry or commit failures roll back the entire command.

## Oracle evidence and limits

The public Rhino 8 command was exercised on shared native-generated 3DM sources,
with native round-trip verification and topology-preserving Rhino insertion.
The retained datasets contain 39 selection/history/smooth-split cases,
85 planar angular-cutoff cases, 64 kinked-surface cases, and 18 straight or
near-straight cases:

- [Command fixtures](../../tools/rhino_oracle/fixtures/merge_edges_command.json)
  and [raw observations](../../tools/rhino_oracle/observations/merge_edges_command.json).
- [Planar angle fixtures](../../tools/rhino_oracle/fixtures/merge_edges_angles.json)
  and [raw observations](../../tools/rhino_oracle/observations/merge_edges_angles.json).
- [Kinked-surface fixtures](../../tools/rhino_oracle/fixtures/merge_edges_kinky_surfaces.json)
  and [raw observations](../../tools/rhino_oracle/observations/merge_edges_kinky_surfaces.json).
- [Straight-edge fixtures](../../tools/rhino_oracle/fixtures/merge_edges_linear.json)
  and [raw observations](../../tools/rhino_oracle/observations/merge_edges_linear.json).
- [Source/observation hashes](../merge-edges-provenance.json) and
  [full raw comparison](../merge-edges-comparison.json).

These are correctness probes, not performance measurements. Every Rhino run
uses an owned private Xvfb display. History probes defer until the setup Python
script has returned: nested Undo can report Success while saying “Nothing to
undo.” EndCommand captures the target command, and post-command snapshots verify
that Undo/Redo actually restore the expected topology. Successful driving macros
do not append Cancel, which would clear selection before testing Undo.

Full raw Rhino equivalence is **not** claimed:

- Some box UV trims use negative parameter intervals in Rhino but zero-based
  intervals natively. Bare-surface insertion also differs in trim intervals
  before cleanup. The regression excludes only these two-control, unit-weight
  segments' affine intervals, preserving their complete UV control definitions.
- 3DM omits phantom outer surface knots; their reconstructed values can differ
  from the source's unused outer knots, already before the command runs.
- For six near-straight observations, native edge uncertainty is `1e-9` while
  Rhino reports zero. This falls within the raw comparison epsilon but is
  explicitly asserted separately, not treated as identical metadata.
- Rhino can split kinked surfaces into additional faces during replacement.
  Native edge cleanup does not split surfaces. Planar trim fixtures isolate the
  edge-angle policy from this separate behavior.
  A [certified face-partition kernel and 107 additional observations](../face-splitting.md)
  now cover this behavior; command integration remains pending and those probes
  are not included in the command equivalence totals below.
- At an exact floating-point angular cutoff, Rhino's cosine comparison and the
  native stable `atan2` comparison can disagree. Near-cutoff raw results are
  retained; the geometry kernel does not round genuine kinks away.

The full raw comparison has **157 matches out of 206**, at absolute epsilon
`1e-9` and relative epsilon `1e-10`: 81/85 planar angle cases, 27/39 command cases,
31/64 kinked-surface cases, and 18/18 straight-edge cases. Four planar differences are
at the exact requested 0.2°, 0.5°, or 1° cutoffs; the native evaluated angles
exceed the cutoff by less than `1e-14` radians. The 39-case regression now compares
full curve definitions and history states, excluding only the twelve cases'
documented outer surface knots or affine UV intervals. Of the kinked-surface cases,
33 cause Rhino to split one face into two (seven edges instead of five).

The new straight-edge cases cover nonuniform knots, weighted quadratic curves,
negative weight gauges, cubic curves, both face senses, and control deviations
from `1e-12` to `1e-6`. Two folded-cubic sources failed native area integration
before a shared artifact could be observed; those errors remain in provenance
and are not counted as Rhino comparisons. Reversing curves are tested directly
in the kernel. These probes do not establish all Rhino simplification policies.

An initial axis-aligned planar probe was rejected by the shared-artifact guard:
3DM reclassified a newly split trim from non-isoparametric to a boundary isocurve.
The final planar matrix uses oblique interior trims and passes the unchanged
round-trip guard; no classification difference is silently normalized.

The [kernel guarantees and limits](../join-edge-cleanup.md) also apply, including
exact UV preservation certificates, bounded work, and degree limits.

Verification: 2,929 release-mode Rust tests, seven opt-in GPU tests, 237 Python
tests, formatting, and Clippy/Rustdoc with warnings denied. The focused debug
kernel, document, command, and oracle regressions also pass.
