# CreateUVCrv

[Command reference](README.md) · [Apply UV curves](apply-curves.md)

`CreateUVCrv` generates a flat World-XY representation of one NURBS surface or
single-face B-rep. Pick a surface, optionally select spatial curves and points,
then press Enter. A preselected surface skips the first stage. Escape restores
the starting selection. Scripts use `CreateUVCrv Surface=object-uuid` with the
optional spatial inputs selected.

The result includes the surface's complete UV rectangle, trimmed outer contours
and holes. A natural rectangular outer loop is emitted once. Stored trim
controls, weights, knots and orientation are retained through affine chart
remapping. Extra on-surface curves use certified pullback; points use the existing
closest-parameter query. Off-surface curves use bounded numerical closest-point
fitting. Inputs beyond tolerance still project and are reported as distant.

Original geometry is retained. Output copies preserve source names, layers,
attributes and user text, and share recreated source groups. Point outputs retain
their groups here. Extra spatial inputs remain selected after creation; Undo
releases them, and Redo selects only the new UV objects. Copied group definitions
survive Undo. Geometry is staged before document insertion.

Include the generated rectangle when using `ApplyCrv` to map the UV objects back
onto the same surface. This preserves the full chart bounds even if trim contours
or extra objects occupy only part of the surface. The viewport construction plane
does not change the World-XY output.

## Sizing and accuracy

Public native observations indicate eight isocurve stations at domain fractions
`0, 1/8, ..., 7/8`; the greatest length in each direction supplies the rectangle
size. The native command uses a coarse fractional curve-length estimate: on a
radius-two cylinder its width is `12.566670662317494`. The implementation uses
the existing accuracy-controlled integration and returns approximately
`12.566370614359172`, the true circumference. This native sizing discrepancy is
retained explicitly, rather than encoded as a primitive-specific correction.

Native comparisons for flat curve coordinates therefore use `1e-3` in the
current fixtures. On the warped and planar test surfaces, rectangle dimensions
agree to floating precision. The CreateUVCrv/ApplyCrv roundtrip test recovers the
original spatial curve within `1e-6`. These checks do not establish `1e-6` flat
coordinate parity for arbitrary surfaces.

The off-surface curve recipe also exposes a projection-contract discrepancy.
The local fit follows closest parameters, including clamping at natural-domain
boundaries; its final flat point is `(4.366062299143245, 6.25)`. Native output
ends at `(4.366062299143245, 6.246813315525544)`, and maximum paired sample
discrepancy is about `0.00686`. This invalid-on-surface-input case is retained
with a `0.01` comparison ceiling and does not establish native projection parity.

Certified on-surface pullbacks retain their kernel degree, sign-coherent weight,
domain and arithmetic limits. Off-surface fitting checks 31 interior stations
per candidate cubic and refines to depth 24; it is a numerical approximation,
not a continuous certificate of the global nearest-point locus. Ambiguous
periodic charts, singular projection branches and discontinuous source curves
may fail or remain unsupported. Multi-face reference picking and inline `SubCrv`
selection are not implemented. Full native sizing and projection parity remain
outstanding.

## Native evidence

The [closed command recipes](../../tools/rhino_oracle/fixtures/create_uv_curves_command.json)
and [raw records](../../tools/rhino_oracle/observations/create_uv_curves_command.json)
cover a warped surface, a plane, cylinder and sphere, optional inputs, off-surface
inputs, preselection, a rotated construction plane, a trimmed warped face and
a planar hole. Captures run at idle on private Xvfb in an empty owned document
with an independent Undo baseline. Public source definitions, trim curves,
isocurve-length diagnostics, output stations, attributes, groups and history
states remain available for replay.

The local Python operation `create_uv_curves` accepts full surface and extra-input
definitions and executes the same document adapter. Its current fixture covers
the nine untrimmed source records; Rust command replay separately reconstructs
the two trimmed sources from their original UV definitions. See
[local records](../create-uv-curves-local.json) and
[provenance](../create-uv-curves-provenance.json).

```sh
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/create_uv_curves_local.json --timeout 600
cargo test -p viboceros-command --release create_uv_curves
cargo test --release --bin viboceros app::tests::create_uv_curves
```

Public reference: [McNeel CreateUVCrv](https://docs.mcneel.com/rhino/8/help/en-us/commands/createuvcrv.htm).
