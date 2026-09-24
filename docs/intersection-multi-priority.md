# Intersection source choice with multiple curves

Twenty-two owned Rhino 8 `GetPoint` captures place three or four straight
curves through one projected crossing. Every capture reports `Intersection` at
the crossing. Each curve has a different height when source ownership matters,
so a different source produces a different 3D target.

| Probe | Inputs | Rhino observations | Native matches |
| --- | --- | --- | ---: |
| Three/four source orders | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_snaps.json) | 1/6 |
| Cursor offsets and order | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_detail_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_detail_snaps.json) | 0/6 |
| Source heights | [6 inputs](../tools/rhino_oracle/fixtures/intersection_multi_depth_snaps.json) | [6 observations](../tools/rhino_oracle/observations/intersection_multi_depth_snaps.json) | 3/6 |
| Line directions | [4 inputs](../tools/rhino_oracle/fixtures/intersection_multi_orientation_snaps.json) | [4 observations](../tools/rhino_oracle/observations/intersection_multi_orientation_snaps.json) | 3/4 |

The Python replay API now accepts observed `Intersection` kinds for its
line/mesh sources. Run a comparison with:

```sh
python3 -m tools.rhino_oracle.point_snap_replay \
  tools/rhino_oracle/fixtures/intersection_multi_snaps.json \
  tools/rhino_oracle/observations/intersection_multi_snaps.json
```

It exits with status 1 for the retained source and height differences. Across
the 22 cases, 7 match completely and 15 select a different source; the
projected crossing itself is found in every case. Reordering curves and varying
depth do not explain Rhino's selection alone. The current pairwise snap
selection needs a measured rule for coincident intersections involving more
than two sources.
