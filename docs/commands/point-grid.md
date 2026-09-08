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
```

Counts describe points, not subdivisions. The initial defaults are 10, 10, and
1. Positive X/Y counts below two are raised to two; ZCount=1 creates only the
base layer. Successful counts are remembered by the command registry, outside
document undo history. Options accept `Name=value` or `Name value`, before or
after the coordinates. Duplicate/unknown options, zero counts, degenerate bases
or heights, non-finite geometry, and more than one million output points are
rejected before document mutation. Creation is one undo step.

Native point order is deterministic: X increases fastest, then Y decreases for
positive height (increases for negative height), then Z advances from the base
to the requested height. The cloud retains its exact grid locations; it is not
a polygon mesh. Existing Explode and point-cloud picking operations apply.

This implementation currently accepts typed two-corner rectangular input.
Rhino's Diagonal, 3Point, Vertical, Center, and picked-height workflows are not
yet implemented. Count-option prompts observed in Rhino 8.32 use `XCount`,
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
The 12-case Rhino 8.32 live comparison passed with maximum coordinate difference
`2.7e-15`. Stored [raw Rhino measurements](../../tools/rhino_oracle/observations/point_matrix_command.json)
replay in an independent one-to-one point-set test at `1e-10`; this test does not
reuse the live comparison's sorting keys. These checks do not establish every
rectangle input mode, arbitrary-scale accuracy, or a Rhino performance comparison.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_command.json --timeout 300
```
