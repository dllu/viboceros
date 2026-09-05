# Full-order NURBS curve interchange

[File formats](file-formats.md) · [Rational numerics](nurbs-numerics.md)

The kernel permits interior knots with multiplicity `degree + 1`. These separate
independent control blocks: they can encode a positional jump, or keep connected
pieces with rational weight scales that cannot safely be combined. OpenNURBS'
`ON_IsValidKnotVector` rejects these knot vectors in a single NURBS object.

## Export policy

`NurbsCurve::try_split_at_full_order_knots` slices independent control/knot blocks
in increasing source-parameter order and clamps their active ends. It does not
fit curves, match homogeneous scales, average endpoints, or relocate closed seams.
The knot scan and block copying are linear in input size for fixed degree.
Curves without these knots are returned unchanged.

Before writing a 3DM file, the IO adapter applies this decomposition to free
NURBS curves and NURBS leaves inside PolyCurves:

- Exactly touching pieces become a native PolyCurve, retaining independent leaf
  types, weight scales, and local and outer parameter intervals.
- Actual positional gaps produce separate objects; no bridging segment is added.
  Original PolyCurve junctions retain the kernel's existing coincidence policy.
- Closed children are written separately because a multi-segment OpenNURBS
  PolyCurve cannot contain a closed child. Groups also split at the segment-count
  limit or if their combined parameter width would overflow.
- A single piece lifted out of a composite receives its original outer domain.
  If a mapped subdivision interval collapses in floating point, export fails
  before replacing the destination file.

Every output object receives the original name, layer, visibility, locking,
display attributes, and ordered group membership. Both hidden and locked state
survive simultaneously; the bridge uses OpenNURBS' public visibility-only parental
control operation because `SetVisible(false)` alone resets the object mode.
Neither source geometry nor document selection or undo/redo history is changed.

`write_3dm_file` returns `ThreeDmWriteReport` with source and written object counts
and the number of adapted source curves. `Export3dm` reports the actual written
count and includes an adaptation notice when needed. This is geometric interchange,
not restoration of the original single-object representation on import.

## Validation

Native tests compare points and first/second derivatives against exact one-sided
source limits, including independent scales `2^-700` and `-2^700`, unclamped
active ends, closed curves, and 1,024 independent spans. File tests check the
source locus, independent control definitions, mapped domains, attributes,
closed-seam edits, wide intervals, and atomic failure. Command tests check export
counts and unchanged selection, geometry, groups, and usable undo/redo history.

`curve_3dm_interchange.json` is a cross-reader probe, not two independent exporters.
The Python client's `compare` mode allocates private temporary paths. Viboceros
writes each file; both native and Rhino readers then inspect that same file.
The native probe first validates 65 parameter-matched samples per written curve
against its source, using exact one-sided limits at decomposed endpoints and
relative `1e-12` without an absolute floor.
Rhino checks curve validity and records native types, domains, 33 samples per
curve and leaf, NURBS definitions, and file attributes. Six cases cover ordinary
lines, connected extreme-weight blocks, positional jumps, closed children, mixed
line/NURBS/arc/polyline composites, and unclamped active ends. Each case includes
all four visibility/locking combinations. Timings measure read-and-inspect work
only, not export versus import.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_3dm_interchange.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

Standalone native runs use automatically removed temporary files. Standalone
Rhino runs require a previously generated `artifact_path`; use `compare` for the
normal managed lifecycle. The client leaves the caller's request unchanged and
removes its artifacts on success or failure.

All six cases passed in Rhino 8 on 2026-09-04 at absolute `1e-10`, relative
`1e-12`; the largest recorded numeric difference was `8.9e-16`. This includes
32 written objects across the six files, with all attribute comparisons exact.

## Remaining limits

This adapter handles free curves, not B-rep edges/trims or full-order surface
knots. Those require topology-aware decomposition. A separate
[numeric range adapter](rational-3dm-range.md) now chooses safe common binary
weight scales before storing homogeneous coordinates and rejects unrecoverable
controls. Kernel active-weight normalization limits still apply. Sampled reader agreement is bounded evidence,
not a proof for every possible curve or floating-point input.
