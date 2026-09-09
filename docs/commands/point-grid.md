# PointGrid

[Command reference](README.md) · [Point-cloud picking](../point-cloud-picking.md)

`PointGrid first-corner opposite-corner [height] [XCount=n YCount=n ZCount=n]`
creates one point cloud filling a rectangular lattice. The base follows the
active construction plane's axes at the first corner; the opposite corner's
normal offset is ignored. Corners may be entered in either order. Omitted
height uses the base's Y width; negative height extends opposite the plane normal.

```text
PointGrid 0,0,0 6,4,0 XCount=7 YCount=5 ZCount=1
PointGrid 0,0,0 6,4,0 -8 XCount=7 YCount=5 ZCount=9
PointGrid 3Point 0,0,0 6,0,2 3,4,5 2 XCount=3 YCount=2 ZCount=2
PointGrid Center 10,20,3 12,24,3 XCount=3 YCount=3 ZCount=2
PointGrid Diagonal 10,20,3 8,24,-1 XCount=3 YCount=2 ZCount=2
PointGrid Diagonal 0,0,3 6,4,3 0,0,5 XCount=3 YCount=2 ZCount=2
```

Counts describe points, not subdivisions. The initial defaults are 10, 10, and
1. Positive X/Y counts below two are raised to two; ZCount=1 creates only the
base layer. Successful counts are remembered by the command registry, outside
document undo history. Options accept `Name=value` or `Name value`, before or
after the coordinates. Duplicate/unknown options, zero counts, degenerate bases
or heights, non-finite geometry, and more than one million output points are
rejected before document mutation. Creation is one undo step.

Enter `PointGrid` with optional count settings and no corners to start picking.
Pick the first and opposite base corners, then pick a height point, type a signed
height, or press Enter to use the base width. Height points are projected onto
the construction-plane normal through the first corner. The first pick captures
the grid's plane; changing viewports afterward does not rotate the grid. Another
parallel view can make normal-direction height picking convenient. Typed point
coordinates use the ordinary active-view coordinate input rules.

Esc cancels without changing geometry or remembered counts. Invalid base picks
or final heights keep the draft for retry; failed completion preserves undo/redo
history. The UI and command share the count parser, and omitted counts remain
unspecified until execution, so interactive commands honor remembered settings.

## Three-point bases

`PointGrid 3Point first-corner edge-end opposite-side-point [height]` defines a
rectangle on the plane through those three points, independent of the CPlane.
The first two points specify the full first edge. The third determines the
perpendicular width, not the opposite corner: its component along the first
edge is discarded. That perpendicular direction is positive Y; X cross Y fixes
the height normal. A negative height extends against that normal. Omitted height
uses the perpendicular width.

Enter `PointGrid 3Point` with optional counts to pick those three base points
before supplying height. Collinear or coincident defining points are rejected by
the kernel's frame validation at document tolerances, and an invalid pick keeps
the preceding points. Height picking uses the three-point plane even when another
viewport is active. Counts remain remembered; the base mode is explicit per
invocation. Numeric width in place of the third point is not yet supported.

## Center-based grids

`PointGrid Center base-center corner [height]` creates a base symmetric about
the first point along the captured CPlane X and Y axes. The corner's normal
offset is ignored. Omitted height uses the **full** Y width, twice the picked
half-width. The first point centers the base, not the height interval; signed
height still starts at that base plane.

Enter `PointGrid Center` with optional counts to pick the center and corner,
then pick, type, or default the height. The base modes are mutually exclusive;
combining `Center` and `3Point`, or repeating either, is an error. The base mode
does not persist to the next command.

Centered endpoints are stored directly, avoiding an overflowing full-span
calculation when all output points remain finite. An explicit finite height can
therefore support such a wide base. An unrepresentable default height or output
coordinate is still rejected before adding geometry.

## Typed diagonal grids

`PointGrid Diagonal first-corner opposite-corner [height-point]` uses all three
CPlane components of the diagonal. Each axis runs from the first corner toward
the second, preserving its sign; X varies fastest, then Y, then Z. When the
computed normal displacement is exactly zero, an explicit height point is
required, and only its normal component relative to the first corner is used.
Otherwise the second corner supplies the height and extra height input is rejected.

This mode currently requires a complete typed command. Interactive Diagonal
picking, numeric-only height, and default height are not supported. The native
exact-zero rule does not claim parity with Rhino's unmeasured near-coplanar prompt
threshold. Diagonal cannot be combined with Center or 3Point.

## Point order and remaining limits

For non-diagonal modes, native point order is deterministic in the base frame: X increases fastest, then Y decreases for
positive height (increases for negative height), then Z advances from the base
to the requested height. The result is a point cloud, not a polygon mesh.
Existing Explode and point-cloud picking operations apply.

This implementation accepts two-corner, three-point, center-based, and typed
diagonal input. Rhino's Vertical workflow is not yet implemented.
The [Diagonal investigation](../point-grid-diagonal.md) records measured behavior
and the remaining prompt uncertainties.
Count-option prompts observed in Rhino 8.32 use `XCount`,
`YCount`, and `ZCount`, unlike the names in the
[online help](https://docs.mcneel.com/rhino/8/help/en-us/commands/pointgrid.htm).

## Verification

The `point_grid_command` Python/native oracle operation uses the permanent
`tools/rhino_oracle/fixtures/point_matrix_command.json` fixture. It runs actual
commands in a private Rhino construction plane and restores the plane and
selection afterward. Cases cover counts, reversed/off-plane corners, signed and
default heights, and World XY/XZ/YZ and oblique frames. Because Rhino's internal
point traversal changes on some oriented planes, the comparison matches lattice
stations before comparing **unrounded** coordinates; duplicate points are not
discarded. It verifies the complete point set, not arbitrary-plane storage-order
parity. Native tests separately check ordered output and transactional failures.
UI tests compare picked and typed results, exercise numeric/default height,
cross-viewport completion, cancellation, tiny valid grids, retryable failures,
and a final count-budget rejection using remembered settings.
The 12-case Rhino 8.32 live comparison passed with maximum coordinate difference
`2.7e-15`. Stored [raw Rhino measurements](../../tools/rhino_oracle/observations/point_matrix_command.json)
replay in an independent one-to-one point-set test at `1e-10`; this test does not
reuse the live comparison's sorting keys. These checks do not establish every
rectangle input mode, arbitrary-scale accuracy, or a Rhino performance comparison.

The three-point fixture additionally checks a planar base, a tilted edge and
off-plane third point, and reversed width with negative height. All three live
Rhino comparisons passed within `1.8e-15`; their
[raw measurements](../../tools/rhino_oracle/observations/point_matrix_three_point.json)
also replay independently at `1e-10`. Native tests check the perpendicular-width
calculation analytically, CPlane independence, and rejected inputs; UI tests
exercise the additional pick stage and its height normal.

The three-case center fixture covers default full-width height, a negative
height with a reflected corner, and an oblique CPlane. Live Rhino comparisons
passed within `2.7e-15`; the
[raw measurements](../../tools/rhino_oracle/observations/point_matrix_center.json)
replay with the same independent point-set check. Native tests additionally
cover finite centered endpoints with an overflowing full span, failure atomicity,
and conflicting base modes. UI tests verify center/corner prompts and height
picking after changing viewports.

Four World XY diagonal cases (54 points) passed a live Rhino comparison with
zero coordinate difference, including source order. Stored measurements replay
as ordered numeric coordinates; native tests additionally cover all eight signed
axis combinations, height-point projection, undo/redo, and rejected inputs.
Four further diagonal cases cover World XZ, World YZ, an oblique CPlane, and
coplanar XZ corners with a height point. All 48 points matched Rhino in source
order within `4.5e-16`; the axis-aligned cases matched exactly. Their
[raw measurements](../../tools/rhino_oracle/observations/point_matrix_diagonal_planes.json)
replay in native ordered tests and independent Python world-coordinate formulas
at `1e-12`. This is sampled plane coverage, not a guarantee at arbitrary scales
or near the unmeasured coplanarity threshold.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_command.json --timeout 300
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_three_point.json --timeout 300
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_center.json --timeout 300
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_diagonal.json --timeout 180
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_diagonal_planes.json --timeout 180
```
