# Taper live preview

[Command guide](commands/taper.md) · [Geometry](taper-geometry.md)

After choosing the axis, the start-distance prompt draws the axis and projected
radius guide. After accepting that distance, moving the cursor adds temporary
object-colored geometry over the selected sources. Clicking uses the same
resolved point as the preview. Sources, attributes, selection and history remain
unchanged until a placement is accepted.

Free radius picks use a plane perpendicular to the axis through the relevant
endpoint. When the viewing line is parallel to that plane, picking falls back
to the construction plane through that endpoint. Object snaps and point filters
retain the common drafting path. Flat orientation uses the first picked radius
direction or the construction plane captured when accepting a numeric start
distance.

The shared deformation display cache prepares curve controls, surface cages,
B-rep wires and mesh edges once per source geometry, tolerance and wire density.
Cursor motion maps these prepared structures without fitting final geometry.
Curves use degree at least three, including when PreserveStructure is enabled;
preserved surfaces keep their controls. Rigid objects reuse their source display
geometry, with grouped objects sharing a placement about combined bounds.

Wireframe, Shaded and Ghosted retain Taper's temporary surface isocurves.
The preview adds wires and points; existing shaded source faces retain their
colors. Copy changes reuse the same pending geometry. Accepted copies reset
the cursor preview and continue from the original sources. Leaving a viewport
retains the last valid preview in the other views. Zero-radius picks remove the
temporary deformation. Cancel removes it and keeps already accepted copies.

## Retained evidence

Private Xvfb captures from Rhino **8.32.26160.13001** retain 30 preview cases:

| Coverage | Witnesses |
| --- | --- |
| Geometry | Points, line, cubic curve, surface, solid box, colored quad mesh |
| Options | Copy and repeat, Rigid and grouped points, Flat with numeric and picked start radii, Infinite, PreserveStructure |
| Display | Wireframe, Shaded, Ghosted; surface interior wires retained in each |
| Input | Start/end prompts, click, cancel, zero snap, Front, Top and Perspective; tilted, spatial and translated axes |

Six earlier cursor diagnostics retain all three candidate plane intersections.
Their actual accepted point geometry distinguishes the axis-normal plane from
both construction-plane candidates. Their provisional SDK predictions for tilted
and spatial cases used the wrong plane; those records remain diagnostic and are
excluded from verified preview comparisons.

Public SDK quick previews constrain controls and samples at `1e-11`, weights at
`1e-14`, and rigid terminal placements at `1e-7`. Native pixels independently
constrain line cages within three pixels and surface wires within four pixels.
Camera replay checks preview/click agreement within `1e-5`. The
[provenance record](taper-preview-provenance.json) hashes helpers, fixtures,
responses and PNGs.

These are sampled witnesses. Complex trims, signed start-radius preview pixels,
arbitrary construction planes, drafting-aid combinations and additional axis
construction modes need more native evidence.

```sh
cargo test -p viboceros --bin viboceros taper
python3 -m unittest tools.rhino_oracle.test_taper_preview
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.taper_preview_input \
  tools/rhino_oracle/fixtures/taper_preview.json \
  --output /tmp/taper-preview.json --scheme VibocerosOracleTaperPreview --timeout 360
```
