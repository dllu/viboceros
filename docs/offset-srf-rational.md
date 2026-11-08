# Rational offset patches

Native `OffsetSrf Loose=No Solid=No` offsets open, untrimmed rational surfaces
with smooth source normals and interior curvature breaks. Smooth patches retain
the inherited UV parameters. Corresponding supports receive a common knot basis
through knot insertion, then join with whole-curve boundary certificates at the
original document tolerance. Reversed faces retain their sense; normal copy,
BothSides and Undo/Redo policies apply.

An independent finite source-normal check includes source and fitted knots,
midpoints and a regular parameter grid. Results exceeding one quarter of the
requested tolerance decline before document edits. The one-million-station
budget is checked before knot refinement. A native success status alone does
not establish the requested accuracy. These stations do not prove a continuous
ideal-offset bound.

Nine fresh Rhino observations cover signed offsets, BothSides, a fine request,
shifted UV domains, reversed faces, V and biaxial breaks, cubic rational
postselection and cancellation. Replays check metadata, pure previews, actual
Undo/Redo and 3DM round trips. Most rational cases also compare subdivision,
normal sense and finite stored-edge/vertex positions at the ordinary epsilon:
`max(1e-5, 2*requested)` model units. Product validation retains document tolerance.

One final BothSides case retains a partition difference: Rhino returns `[4,3]`
faces while Viboceros returns `[4,4]`. Its finite surface images, spatial edges
against the other shape, and exterior boundary sets use the same positional
limit. The additional internal edge remains; identical internal subdivisions and
normal-angle/derivative parity are unqualified. Earlier qualification records
retain their original rational-case decline and revision pins.

Trimmed curvature breaks, G1 creases, closed supports and solid walls retain
their existing paths and exclusions. Portable builds retain their curved-offset
limits. Full matrix qualification is in progress.

```sh
cargo test -p viboceros-smlib --features native --test surface_offset
cargo test -p viboceros-command --features native-smlib --test offset_surface_tolerance rational_
python3 -m tools.rhino_oracle.check_offset_srf_rational
```
