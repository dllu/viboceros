# ArcBlend

[Curve commands](curves.md) · [Command reference](README.md)

`ArcBlend` connects two open curve ends with two tangent circular arcs. Pick
each source near the desired end, adjust options, then press Enter to commit
the preview. [Adjustment handles](arc-blend-handles.md) support endpoint sliding,
positive ratios and held drags. [Radius differences](arc-blend-radius.md) support
parallel forward tangents. Esc cancels without changing geometry or pending redo history.
Source selection clears when the command starts. The result uses the current
layer, with one undo record for the blend and any source edits.

Options currently supported:

- `AlternateSolution` toggles the other signed tangent-length solution when
  available; `AlternateSolution=Yes|No` sets it explicitly.
- `Trim=Yes|No` controls source trimming; the default is No.
- `Join=Yes|No` becomes available with Trim enabled. Yes replaces two distinct
  ordinary source curves with the joined source portions and blend.
- `Ratio=positive-number` sets the ratio of the two arc control-polygon
  lengths; the default is 1. Zero restores the last radius target, initially
  zero. This is a Viboceros scripting option.
- `RadiusDifference=number` locks the first arc's radius minus the second's
  for parallel forward tangents. Interactively, enter `RadiusDifference`,
  then the number. Positive ratios release the lock.
- `Parameter1=t` and `Parameter2=t` move the contacts within the original
  source parameter domains, retaining the selected end orientations. These
  are Viboceros scripting options; they require both source picks first.

For reproducible scripts, use object IDs and world-coordinate source picks:

```text
ArcBlend Curve=id1 0,0,0 Curve=id2 5,4,0 Ratio=1 Trim=No
```

An interactive source can also be entered as `Curve=id`, followed by a point.
`Edge=id,index` accepts a read-only B-rep edge with `Trim=No`. Ordinary
drafting point syntax applies to interactive typed picks. Closed sources,
degenerate two-arc solutions, joined self-blends and surface-edge trimming
are rejected. Source geometry or tolerance changes invalidate stored picks.

The geometry API is `viboceros_geometry::try_arc_blend`, with
`ArcBlendOptions { ratio, alternate }`. It normalizes direction vectors and
solves an independent tangent-length equation, retaining two exact circular
arc segments. NURBS output uses the existing public OpenNURBS conversion.

[Validation](../arc-blend-validation.json) retains 53 Rhino observations:
20 SDK constructions with independent point/tangent samples, 17 SDK
rejections, six matched command workflows and ten diagnostics. Both kernel
builds compare coefficients, sampled geometry, attributes, selection,
undo/redo and 3DM round trips. Egui mouse tests run without a native window;
Rhino captures use private Xvfb sessions. Reproduce the checks with:

```sh
python3 -m tools.rhino_oracle.check_arc_blend
```

This is a first implementation. General nonparallel radius solving,
SubCrv and remembered settings between commands remain
unsupported. Native command comparisons cover endpoint contacts on lines;
interior parameter edits and edge references lack native qualification.
Three raw non-unit SDK direction cases have discontinuous native join
tangents and are retained as diagnostics. Seven command discovery cases
contain incomplete inputs, rejected options or unattained radius targets.
Neither group counts as a passing geometry comparison.
