# Intersection source choice with multiple curves

Twenty-two owned Rhino 8 `GetPoint` captures place three or four straight
curves through one projected crossing. Every capture reports `Intersection` at
the crossing. Each curve has a different height when source ownership matters,
so a different source produces a different 3D target.

| Probe | Inputs | Rhino observations | Native matches |
| --- | --- | --- | ---: |
| Three/four source orders | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_snaps.json) | 6/6 |
| Cursor offsets and order | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_detail_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_detail_snaps.json) | 5/6 |
| Source heights | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_depth_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_depth_snaps.json) | 6/6 |
| Line directions | [4 inputs](../tools/rhino_oracle/fixtures/intersection_multi_orientation_snaps.json) | [4 observations](../tools/rhino_oracle/observations/intersection_multi_orientation_snaps.json) | 0/4 |

The Python replay API now accepts observed `Intersection` kinds for its
line/mesh sources. Run a comparison with:

```sh
python3 -m tools.rhino_oracle.point_snap_replay \
  tools/rhino_oracle/fixtures/intersection_multi_snaps.json \
  tools/rhino_oracle/observations/intersection_multi_snaps.json
```

It exits with status 1 for the retained source and height differences. Across
the 22 cases, 17 match completely and five select a different source; the
projected crossing itself is found in every case. Straight-wire crossings at
the same screen point are grouped, and the source closest to the cursor is
chosen when at least three distinct non-mesh objects participate. This
resolves the measured source-order and depth cases while retaining pairwise
behavior for two sources. The [eight mouse-approach
inputs](../tools/rhino_oracle/fixtures/intersection_multi_motion_snaps.json)
and [Rhino observations](../tools/rhino_oracle/observations/intersection_multi_motion_snaps.json)
try each one-pixel cardinal approach on two of the triple crossings. Both
returned the same source in all four directions; native replay matches four
of the eight. Rhino's remaining five source choices cannot be explained by
cursor distance, source order, depth, or one-pixel approach alone.
