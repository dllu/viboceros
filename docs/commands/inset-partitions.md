# Inset edge partitions and curvature joins

[Curved boundaries](inset-curves.md) · [Cusp loops](inset-cusps.md)

Splitting a smooth curved boundary into several B-rep edges preserves the
annular border for both `CreaseCorners` settings. Original vertices and edges
remain in the outer boundary. Grouping uses public OpenNURBS G2 classification,
including its relative curvature tolerance. Six asymmetric-ellipse controls
distinguish small accepted curvature changes from joins that produce strips.
Reassigning identical source curves to new curve-table entries leaves grouping
unchanged; the original split controls already have independent entries.

With `Loose=On`, a smooth partitioned loop offsets its joined original basis.
New cap kinks can split that loop into several edges without creating border
strips. With `Loose=Off`, cusp-loop removal operates across smooth source joins.
`CreaseCorners=No` also supports closed tangent-continuous chains whose cusps
cross curvature joins. A surviving stored seam remains a cap vertex; an erased
seam disappears.

`CreaseCorners=Yes` retains strips at curvature breaks. Straight subdivisions
retain triangular strips even when the cap simplifies to one straight edge.
Their connectors choose the nearer retained cap endpoint. At exactly symmetric
midpoints, roundoff in native fitted endpoints can select the opposite endpoint
from the kernel's deterministic following-endpoint tie rule. Three captured SDK
cases expose this topology difference; their strict regressions remain ignored.

Creased cusp fans remain unsupported. Four native SDK successes construct 7–79
faces from fitted cap pieces and triangular fans to old corners. The kernel
returns a cusp error without modifying the source. Strict ignored regressions
retain those native results for future implementation. Additional
[fan diagnostics](../geometry/inset-fans.md) freeze 68 cases covering connector
crossings, degree changes, tolerance refinement and independent offset APIs.

| Capture | Cases | Active full comparisons | Pending native successes |
| --- | ---: | --- | --- |
| Partitioned SDK loops | 40 | 28 valid, 12 null collapses | 0 |
| Partitioned commands | 114 | 78 successes, 36 failures | 0 |
| Continuity SDK controls | 28 | 17 valid, 6 null collapses | 4 cusp fans, 1 midpoint |
| Continuity commands | 84 | 51 successes, 18 failures | 12 fan modes, 3 midpoint modes |
| Curvature thresholds | 6 | 6 valid | 0 |
| Straight subdivisions | 12 | 10 valid | 2 midpoints |

All captures use a private display, preserve exact producer snapshots, and are
immutable. Comparisons check complete oriented loops, edge incidence and spatial
witnesses. Command replays also check attributes, selection, undo/redo and 3DM.
Independent tight fits allow `2e-6` at `1e-6` model tolerance; Loose uses `1e-10`.
Strict comparisons require matching connector incidence.

```sh
cargo test -p viboceros-oracle --test inset_surface
cargo test -p viboceros-oracle --features native-smlib --test inset_surface
python3 -m unittest tools.rhino_oracle.test_inset_multiedge tools.rhino_oracle.test_inset_continuity
```
