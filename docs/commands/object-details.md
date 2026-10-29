# What and List

`What` reports the selected objects' IDs, names, layers, colors, visibility,
locking, groups and user text, followed by geometry types and representation facts.
Curve domains, NURBS degrees and control counts, mesh/SubD counts, and block
definition names and IDs come from the stored model.
Point coordinates use world axes and document model units.

`List` adds technical geometry details from OpenNURBS' existing dump routines,
using the same native constructors as 3DM export and `Check`. Instance reports
include the actual definition UUID and placement. The command does not expand
or modify the definition. Unavailable native details are reported explicitly.

Select objects before running either command, or run the command, pick objects
and press Enter. Esc cancels picking. Both commands accept no arguments and keep
geometry, attributes, selection and Undo/Redo unchanged. Reports appear in the
command history in selection order and work in portable and native builds.

Technical output describes Viboceros' native geometry representation; its text
format is not a copy of Rhino's property window. Reports retain up to 256 KiB
overall and 128 KiB of native text per object, with explicit truncation markers.
The shared diagnostic transfer limit is 256 MiB after payload preparation.
Use [Check](check.md) for structural validity and [measurement commands](measurements.md)
for lengths, areas and volumes.

```sh
cargo test --frozen -p viboceros-command --lib object_details
cargo test --frozen -p viboceros-io three_dm::
cargo test --frozen --bin viboceros object_details
```

Reference: [Rhino What and List](https://docs.mcneel.com/rhino/8/help/en-us/commands/what.htm).
