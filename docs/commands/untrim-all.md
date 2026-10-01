# Remove all surface trims

[Command reference](README.md)

`UntrimAll [KeepTrimObjects=Yes|No]` removes exterior trims and holes from
selected standalone surfaces. It restores the complete underlying NURBS surface
without fitting, changing its knots, control points, weights, or parameter
domains. Face orientation, object identity, attributes, and groups are preserved.
New boundary edges use the original surface parameter intervals, including
shifted and negative domains; reversed north/west edges negate those intervals.

```text
UntrimAll KeepTrimObjects=Yes
```

With no preselection, the command prompts for surfaces and offers
`KeepTrimObjects` while selecting. Enter completes the edit and releases the
picked selection. With eligible surfaces already selected, bare `UntrimAll`
asks whether to keep trim objects; answering completes the edit and preserves
preselection. An explicit option on the command line executes directly.

`KeepTrimObjects` starts at No and is remembered by the command registry.
Yes creates separate, unselected boundary curves with current-layer defaults,
without the source's name, object color, or group memberships. Each surface is
renewed after its retained curves in document order. A single closed boundary
edge keeps its NURBS representation and native interval; joined straight pieces
use chord-length polyline parameters. Seam and singular trims create no curves.
Even an already untrimmed surface records an undoable replacement. All source
geometry is staged before editing, and one Undo restores the complete command.

Whole polysurfaces are rejected, matching Rhino's whole-object filter.
Polysurface face subobject selection, hatches, and automatic crease-splitting
settings remain pending. [UntrimBorder](untrim-border.md) preserves interior
holes while restoring the exterior. `Untrim` and `UntrimHoles` remain pending.

## Verification

The [source fixture](../../tools/rhino_oracle/fixtures/untrim_all.json) and
[Rhino 8.32 capture](../../tools/rhino_oracle/observations/untrim_all.json) retain
100 private-Xvfb cases: exterior trims, holes, circular rational boundaries,
paraboloids, oblique planes, reversed faces, multiple sources, periodic cylinder
seams, a collapsed boundary, kinked degree-one boundaries, and noncurrent-layer
attributes, multiple holes, and shifted/negative parameter domains. Both
selection workflows and both retention choices are covered.

Rust replay compares complete source and result definitions, topology,
chronological order, selection, names, colors, layers, and memberships for 96
cases with absolute epsilon `1e-9`. The remaining four box cases compare
whole-polysurface rejection and unchanged geometry separately in each engine:
the independent box constructors have different face and edge tables. Four
capped two-face paraboloid cases also verify rejection. Python checks the full
explicit source nets and boundaries, source-only fixture regeneration, and
cleanup after failures. Application tests exercise picking, preselection,
options, cancellation, and Undo.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/untrim_all.json --timeout 300
cargo test --release -p viboceros -p viboceros-command -p viboceros-oracle untrim
python3 -m unittest tools.rhino_oracle.test_untrim
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_all.json tools/rhino_oracle/observations/untrim_all.json
```

The command follows McNeel's [UntrimAll documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/trim.htm#UntrimAll).
These captures establish the stated cases, not all surface representations,
interactive face subobjects, crease settings, or hatch behavior.
