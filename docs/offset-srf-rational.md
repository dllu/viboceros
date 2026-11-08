# Rational offset patches

Native `OffsetSrf Loose=No Solid=No` can offset an open, untrimmed rational
surface with smooth normals and interior curvature breaks. Each smooth patch
is fitted in its inherited UV parameters, then joined at the original document
tolerance using whole-curve boundary certificates. Reversed faces retain their
orientation. The command's normal copy, BothSides and Undo/Redo policies apply.

A bounded independent source-normal check includes source and fitted knots,
midpoints, and a regular parameter grid. Results exceeding one quarter of the
requested tolerance decline before document edits. A native success status alone
is insufficient: very fine requests can exceed the fitter's resolution.
This finite admission check does not prove a continuous ideal-offset bound.

The retained `offset_th_rational` Rhino case is now covered by a focused command
test, including metadata, actual Undo/Redo and a 3DM round trip. Negative offsets,
reversed faces, BothSides and fine-request atomic rejection have independent
regressions. These targeted checks are not a new full native parity qualification;
wider captures and UI qualification remain pending. Earlier qualification records
retain their original rational-case decline and revision pins.

Trimmed supports with curvature breaks, creases, closed supports, and solid walls
retain their existing paths and exclusions. Portable builds retain their existing
curved-offset limits.

```sh
cargo test -p viboceros-smlib --features native --test surface_offset
cargo test -p viboceros-command --features native-smlib --test offset_surface_tolerance rational_
```
