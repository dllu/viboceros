# ApplyCrv / ApplyCurves

[Command reference](README.md) · [Certified surface images](../certified-surface-pushups.md)

`ApplyCrv` maps World-XY curves and points onto one NURBS surface or a selected
B-rep face. `ApplyCurves` is an alias. Start the command, select sources, press Enter,
then pick the target surface. Preselected curves and points skip the first stage.
Source picking respects groups; the target remains a read-only reference.
Escape restores the original selection. Picking the target completes the command.

For scripts, select the source objects and run:

```text
ApplyCrv Surface=target-object-uuid [Face=index]
```

The combined tight XY bounds of eligible sources map affinely onto the target's
complete natural U/V domain. Include a UV rectangle to establish the desired
mapping bounds: without it, the remaining selected curves and points expand to
fill the surface. The construction plane does not change this World-XY mapping.
Inputs farther from World XY than document absolute tolerance are skipped and
do not affect the mapping rectangle. Accepted near-plane sources are flattened.
An exactly zero-width or zero-height rectangle succeeds with no new geometry.
It creates no Undo entry; `Undo` reports nothing to undo and clears the picks.

The command keeps original objects and preserves output names, layers, user text
and attributes. Curve outputs receive independent copies of their source groups;
point outputs are ungrouped. Corresponding group definitions are allocated for
point-only input too and survive Undo as empty definitions. Successful output is
selected. Undo removes the new objects and releases source/target selection;
Redo restores the same objects and output selection. All fallible geometry is
staged before insertion, so a failed certificate rolls back the command.

## Geometry and limits

Analytic curves, polylines, polycurves and NURBS are converted with their native
parameter maps. Source bounds use the existing tight-bound query at a stricter
numerical tolerance. Scalar remapping retains overflowing widths, subnormal
intervals, reversal and extrapolated control positions through exact rational
arithmetic and one final rounding. Mapped UV controls and original weights/knots
feed the certified surface pushup kernel. Every returned spatial curve is proved
against that stored mapped UV spline at document absolute tolerance.

The bound does not certify an ideal exact-extremum rectangle or an unrounded
XY-to-UV affine transformation. The tight-bound query uses floating arithmetic,
and control remapping can round. Extreme charts may therefore fail qualification
or differ from native mapping; the checked native cases below have ordinary
coordinate ranges. Kernel degree, sign-coherent weight, natural-domain and work
limits also apply. A trimmed target uses its underlying surface without clipping
the output to trim loops. [Multi-face references](../uv-face-references.md) now accept viewport hits and
explicit face indices. Inline `SubCrv` selection is not implemented. [`CreateUVCrv`](create-uv-curves.md) can now generate the
reference UV rectangle, trim contours and optional flattened spatial inputs.

## Native evidence

The [13 closed recipes](../../tools/rhino_oracle/fixtures/apply_uv_curves_command.json)
and [raw observations](../../tools/rhino_oracle/observations/apply_uv_curves_command.json)
exercise rectangle and implicit bounds, points, rational curves with and without
a rectangle, planar tolerance, excluded distant inputs, degenerate rectangles,
preselection, a periodic cylinder, and a rotated construction plane. Each recipe
runs the public `ApplyCrv` command in an idle, empty owned document on private
Xvfb, with an independent Undo baseline. No proprietary implementation is read.

Rust tests reconstruct the original surface/input definitions, execute the actual
document adapter, compare all 561 native curve stations and nine point outputs
at `1e-6`, check source purity and metadata, and verify output Undo/Redo and group
definition retention. Application tests cover both selection stages, target
clicks, aliases, cancellation, grouped source picking, and construction-plane
independence. This establishes the checked cases rather than arbitrary command
or topology parity.

The local Python operation `apply_uv_curves` accepts full source definitions and
executes the same adapter, recording before/after/Undo/Redo states. Its timing
covers command execution and proof, excluding setup and snapshots. Native
command capture timing is unmeasured; these records establish no performance
parity. See [provenance](../apply-uv-curves-provenance.json) and
[local records](../apply-uv-curves-local.json).

```sh
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/apply_uv_curves_local.json --timeout 600
cargo test -p viboceros-command --release apply_curves
cargo test --release --bin viboceros app::tests::apply_curves
```

Public references: [ApplyCrv](https://docs.mcneel.com/rhino/8/help/en-us/commands/applycrv.htm),
[CreateUVCrv](https://docs.mcneel.com/rhino/8/help/en-us/commands/createuvcrv.htm), and
[Surface.Pushup](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_Pushup.htm).
