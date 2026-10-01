# Restore exterior surface boundaries

[Command reference](README.md)

`UntrimBorder [KeepTrimObjects=Yes|No]` restores the natural exterior boundary of
selected standalone surfaces while preserving interior holes. Underlying NURBS
surfaces, face orientation, hole curves, UV trims, and their tolerances remain
exact. Unused vertices and edges are removed in source order, and natural
boundary topology is appended after the retained hole topology. Periodic seams
and collapsed sides retain the natural surface's seam and singular topology.
New boundary edges use native surface intervals, including shifted and negative
domains, with negated intervals for reversed north/west edges.

```text
UntrimBorder KeepTrimObjects=Yes
```

Without preselection, pick surfaces, set `KeepTrimObjects` during selection,
then press Enter. With preselected surfaces, bare `UntrimBorder` asks whether to
keep exterior trim objects; Yes, No, or Enter completes the command. An explicit
option executes directly. Each command registry remembers this option separately
from `UntrimAll`.

Yes retains exterior curves with current-layer defaults and no source name,
color, or groups. Holes remain in the surface and create no new curves. New
curves are unselected and precede each renewed source in document order.
Source identity, attributes, groups, and preselection are preserved;
command-first picks are released. One Undo restores the complete edit, including
all retained curves and object ordering. Already natural exterior boundaries
still create undoable replacements.

Whole polysurfaces are rejected. Face subobject selection, hatches, and automatic
crease-splitting settings remain pending. `UntrimHoles` needs hole-edge selection,
the All option, and maximum-edge-length filtering; it remains pending.

## Verification

The [source fixture](../../tools/rhino_oracle/fixtures/untrim_border.json) and
[Rhino capture](../../tools/rhino_oracle/observations/untrim_border.json) contain
100 private-Xvfb cases: planar and curved surfaces, rational boundaries, reversed
faces, multiple holes, non-default parameter domains, periodic seams, singular
boundaries, degree-one kinks, multiple sources, and noncurrent-layer attributes.
Both selection workflows and both retention choices are recorded.
Rust and Python replay compare complete geometry and attributes for 96 cases
with absolute epsilon `1e-9` and relative epsilon zero. Four independent box
factories compare whole-object rejection and unchanged geometry within each
engine, with no claim that their source topology tables match. Four capped
two-face sources also verify rejection. Tests exercise hole preservation,
permuted multi-edge topology, closed-surface seams, option memory, picking,
cancellation, and one-step Undo/Redo. Corrupted hole weights and UV knots fail
the Python replay; only the original source request reaches the native engine.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/untrim_border.json --timeout 300
cargo test --release -p viboceros-geometry -p viboceros-command -p viboceros-oracle -p viboceros untrim
python3 -m unittest tools.rhino_oracle.test_untrim tools.rhino_oracle.test_untrim_border
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_border.json tools/rhino_oracle/observations/untrim_border.json
```

McNeel's [UntrimBorder documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/trim.htm#UntrimBorder)
describes the exterior-only command. The captures cover the stated source
representations and workflows; they do not establish the pending workflows.
