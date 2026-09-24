# Intersection source choice with multiple curves

Fifty-one owned Rhino 8 `GetPoint` captures place three or four straight
curves through one projected crossing. Every capture reports `Intersection` at
the crossing. The curves have different heights where source ownership matters,
so a different source produces a different 3D target.

| Probe | Inputs | Rhino observations | Native matches |
| --- | --- | --- | ---: |
| Three/four source orders | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_snaps.json) | 6/6 |
| Cursor offsets and order | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_detail_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_detail_snaps.json) | 6/6 |
| Source heights | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_depth_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_depth_snaps.json) | 6/6 |
| Line directions | [4 inputs](../tools/rhino_oracle/fixtures/intersection_multi_orientation_snaps.json) | [4 observations](../tools/rhino_oracle/observations/intersection_multi_orientation_snaps.json) | 4/4 |
| Cardinal cursor approaches | [8 inputs](../tools/rhino_oracle/fixtures/intersection_multi_motion_snaps.json) | [8 observations](../tools/rhino_oracle/observations/intersection_multi_motion_snaps.json) | 8/8 |
| Vertical-source cursor sweep | [9 inputs](../tools/rhino_oracle/fixtures/intersection_vertical_sweep_snaps.json) | [9 observations](../tools/rhino_oracle/observations/intersection_vertical_sweep_snaps.json) | 9/9 |
| Rotated source directions | [4 inputs](../tools/rhino_oracle/fixtures/intersection_rotated_priority_snaps.json) | [4 observations](../tools/rhino_oracle/observations/intersection_rotated_priority_snaps.json) | 4/4 |
| Coarse vertical slopes | [4 inputs](../tools/rhino_oracle/fixtures/intersection_near_vertical_coarse_snaps.json) | [4 observations](../tools/rhino_oracle/observations/intersection_near_vertical_coarse_snaps.json) | 4/4 |
| Tiny vertical slopes | [4 inputs](../tools/rhino_oracle/fixtures/intersection_near_vertical_tiny_snaps.json) | [4 observations](../tools/rhino_oracle/observations/intersection_near_vertical_tiny_snaps.json) | 4/4 |

The Python replay API now accepts observed `Intersection` kinds for its
line/mesh sources. Run a comparison with:

```sh
python3 -m tools.rhino_oracle.point_snap_replay \
  tools/rhino_oracle/fixtures/intersection_multi_snaps.json \
  tools/rhino_oracle/observations/intersection_multi_snaps.json
```

All 51 captures match in kind, source, and 3D point within `1e-9`. Straight-wire
crossings at the same screen point are grouped. When at least three distinct
non-mesh objects participate, a screen-vertical wire is excluded if a
nonvertical wire is available; the closest remaining source to the cursor is
chosen. The nine-position sweep found that Rhino never chose the vertical
wire, even when the cursor lay on it. Four rotated-source cases confirmed
that the preference follows screen direction. Eight further cases show that
a slope as small as `1e-12` is eligible, so native vertical detection allows
only projection roundoff. The eight cardinal approach cases also match.
Pairwise behavior for two sources is unchanged. Smaller representable slopes,
other camera projections, and curved multi-object crossings still need separate
measurement.
