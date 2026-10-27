# Automatic Sweep1 placement

[Sweep1](sweep1.md) · [Rhino oracle](oracle.md)

Select the rail first, then profiles in traversal order, and run `Sweep1`.
Alternatively, select only the profiles and run `Sweep1 RailName=Rail` or
`Sweep1 RailId=id`. Starting the command before selection opens a curve picker;
Enter builds one B-rep and Escape restores the previous selection. Options such
as `RefitRail=Yes` and `FrameStyle=Roadlike Axis=0,1,0` can be supplied inline.

Scripts with explicit `Parameters=` keep the previous last-selected-rail policy.
Names resolve case-insensitively and must be unique. Rail and profiles are retained;
construction is atomic and undoable. Profile directions and seams are retained.

Automatic placement finds a near-minimum between each complete profile and rail.
Linux native builds reuse SMLib's minimum solver, without computing distance
maxima. Exact rational span hulls independently certify the returned distance
bracket. Portable builds use the same certificate with cached numerical point
queries as initial proposals. The value error is bounded by document tolerance;
this does not prove parameter uniqueness or choose among coincident branches.
Explicit stations remain useful for ambiguous placement. For analytic arcs the
command applies the separately measured [transport parameter policy](sweep1-arc-parameters.md)
after obtaining the geometric witness; the final transport station need not be
that nearest-point parameter.

Profiles keep their input order. Descending stations reverse the rail; mixed or
coincident stations fail. A single profile chooses forward or backward traversal
according to which half of the rail's arc length contains its station. With
`RefitRail=No`, an existing interior Greville station can anchor the complete rail;
otherwise the sweep begins at the profile. Partial intervals use a cubic refit,
including the measured straight-rail interior cases with `RefitRail=No`.
The geometry model rechecks corners and seam continuity when extending its interval.
[Rail continuity](sweep1-rail-intervals.md) is checked on the original source so
that a tiny rounded trim cannot turn a smooth join into a false corner.

## Evidence and limits

The earlier [automatic validation record](sweep1-automatic-validation.json) freezes
the implementation at commit `8b0df1328`. The [arc validation](sweep1-arc-parameters-validation.json)
records the subsequent parameter-policy correction. Seven captured straight-rail command surfaces are compared
through both complete rational boundary curves. Matching V weight rows make the
surface affine between those curves, so these bounds cover the entire surfaces.
The same constructions exercise ordered selection, rail names and rail IDs.

Eight additional curved command captures include start, interior, end and reversed
profiles on an arc and a spatial cubic. Bidirectional world-space samples are
numerical comparisons, not complete surface proofs. The spatial reversed case
retains a sampled discrepancy above `1e-6`; a separate complete
correspondence to the explicit-station model distinguishes that existing behavior
from automatic placement. That numerical comparison alone does not prove
different continuous images. A separate [surface-distance certificate](surface-distance-bounds.md)
now proves an image gap above `2.376e-6` for the reversed spatial model constructed
at the capture's `1e-7` tolerance. Further station/direction captures check twelve pairs
of rail-boundary endpoints; they do not qualify all of those surface interiors.
The original non-Greville arc-at-thirty-degrees case now has a complete-image
regression under the corrected transport policy.

The query supports common-sign rational weights, degree at most 16 and at most
4096 controls per curve. Exact arithmetic, subdivision depth and pair storage
are bounded; unsupported or inconclusive queries fail before editing the document.
Distance certification does not certify surface topology or frame compatibility.
General branch choice, profile seam alignment, closed-rail holonomy and rail
miters remain unfinished.

The earlier [refactor and placement record](sweep1-placement-validation.json)
measures the code at commit `a308fd29d`, before automatic placement. Its archived
SDK/command datasets remain unchanged, including their differing end-profile and
reversed-order behavior. Authored `parameters` in those older fixtures are hints
for the explicit harness, not measured Rhino stations.

```sh
cargo test -p viboceros-command --lib sweep:: --frozen
cargo test -p viboceros-geometry --test curve_minimum --test sweep_anchoring --frozen
python3 -m unittest tools.rhino_oracle.test_sweep1_automatic
```
