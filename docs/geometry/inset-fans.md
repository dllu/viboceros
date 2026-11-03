# Inset fan diagnostics

[Inset partitions](../commands/inset-partitions.md) · [Curve networks](curve-networks.md)

Creased cusp fans remain pending in the production Inset path. Six immutable
Rhino 8.32 capture families extend the earlier continuity cases. They were run
on a private Xvfb display and retain content-addressed producer snapshots.
`tools/rhino_oracle/test_inset_fans.py` checks all recipes, snapshots, source
preservation, native validity, and the recorded topology.

| Family | Cases | Purpose |
| --- | ---: | --- |
| `inset_fan_basis` | 12 | Six outward null results and six expanding-hole outputs |
| `inset_fan_interior` | 15 | Radius, distance, degree and subdivision controls |
| `inset_fan_refinement` | 14 | Fit tolerance varied independently of source construction |
| `inset_fan_fit` | 15 | Direct curve-offset controls |
| `inset_fan_surface_fit` | 12 | Surface-offset controls |
| `inset_fan_commands` | 6 | Curves, separate and joined crossed-cap outputs |

The basis family and three expanding-hole command modes now have full production
comparisons; see [closed-edge networks](../commands/inset-closed-networks.md).
The remaining families retain diagnostic evidence without a claim of complete
Rust/native agreement.
Direct curve and surface offsets return cubic curves for these quadratic inputs;
Inset retains a quadratic cap basis. Substituting either public offset API does
not reproduce that basis. Fit tolerance also changes native curvature partitions
and resulting fan topology.

Connector crossings require a full curve network. For a stadium with ellipse
radius 3 and inset distance 1.4, native output has 129 faces: 94 triangles,
16 quads, 16 pentagons, two 21-edge faces and one 48-edge face. One face per
fitted cap segment cannot represent those cells. Degree elevation changes the
native result substantially: the cubic source at the same distance has nine
faces. Source subdivisions retain their extra original boundary edges.

An expanding stadium hole supplies a separate crossing control. A connector
from source seam `(4,2)` to cap corner `(-4,-3.5)` crosses the source bottom at
`(-20/11,-2)`. Native output inserts that vertex. Pre-splitting every source edge
at intrinsic curvature breaks does not explain this partition.

Curves output preserves the complete fitted cap loop and original connector
segments before network subdivision. The stadium control returns 94 connectors
and one cap, while split mode returns 129 faces. The expanding-hole control
returns two cap loops and four connectors, or six separate faces. Command
captures also preserve attributes, selection, undo/redo and source 3DM archives.

The production path now assembles regular closed-edge networks with source-edge
lineage and all output modes. Cusp fans still need qualified fitting and
connector correspondence against their complete native outputs.

```sh
python3 -m unittest tools.rhino_oracle.test_inset_fans
```
