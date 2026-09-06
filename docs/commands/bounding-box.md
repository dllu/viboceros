# Bounding boxes

[Surfaces and solids](surfaces.md) · [Tight bounds](../trimmed-face-bounds.md) · [Oracle](../oracle.md)

```text
BoundingBox CoordinateSystem=World Cumulative=Yes Output=Solids
BBox CoordinateSystem=CPlane Cumulative=No Output=Curves
```

`BBox` aliases `BoundingBox`. Options are case-insensitive and accept either
`Name=Value` or `Name Value`, including underscore-prefixed script tokens.
Defaults are World, cumulative, and solids; options are not sticky.

World uses the world axes; CPlane uses the active viewport's actual construction
plane. The enclosure is axis-aligned **in that coordinate system**, with corners
placed back in world space. Reports give minimum, maximum, and dimensions in the
chosen coordinates. `Cumulative=No` computes one enclosure per selected object.
This follows the [documented coordinate-system and cumulative options](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/boundingbox.htm).

| Output | Three-dimensional enclosure |
| --- | --- |
| `Solids` | One closed B-rep: eight shared vertices, twelve edges, six bilinear faces. |
| `Meshes` | One closed mesh: 24 face-local vertices and six outward quads, triangulated internally for rendering. |
| `Curves` | Six closed rectangle polylines in one group per box. |
| `None` | Coordinate report only; no geometry, groups, or undo step. |

Two-dimensional enclosures produce one closed rectangle for every geometry
output, **including Meshes**, as observed in Rhino 8. This differs from the
manual's description of a planar mesh output. The collapsed coordinate is the
minimum coordinate, not the midpoint. Line/point enclosures fail, including
`Output=None`. Original geometry, attributes, and selection are retained;
new objects are unselected on the current layer.

## Geometry and failure policy

`viboceros-command::bounding_box` owns parsing, coordinate transforms, reports,
and staged construction. It queries `Geometry::tight_bounds`, including analytic
curve extrema, rational NURBS extrema, and retained trimmed-face interiors;
control nets and display tessellations are not substituted for tight geometry.
The [curve](../curve-bounds.md), [surface](../surface-bounds.md), and
[trimmed-face](../trimmed-face-bounds.md) numerical limits still apply.

Each cumulative/per-object query uses a geometry-local working origin. Sources
are translated before rotation, avoiding cancellation from a distant CPlane
origin or a single combined affine matrix. Geometry dimensions are computed
before adding the report-coordinate offset. Very distant report coordinates
can nevertheless lose small differences in floating point; the separately
computed dimensions and actual output geometry remain useful.

An axis collapses when its extent is at most the document's absolute tolerance.
All boxes are validated and constructed before document mutation. A degenerate
individual input, unresolved rational pole, ambiguous trim, exhausted bounds
budget, or unrepresentable transform fails the whole command without partial
objects/groups or an undo entry. This intentionally differs from Rhino's
partial output for mixed valid/degenerate individual selections.

## Verification and retained differences

`bounding_box.json` has 57 actual Rhino command comparisons covering every
output, cumulative/individual selection, World and arbitrary CPlanes, planar
and degenerate inputs, point clouds, mesh vertices, lines, and positive/mixed-
weight NURBS curves and surfaces. All agree at absolute `1e-8`, relative `1e-12`;
maximum observed coordinate error is `7.84e-10`.

`bounding_box_diagnostics.json` retains 26 failing comparisons at the same
epsilon; they are not counted as passing compatibility tests:

- Six ordinary disk-face cases differ by `1.1921e-8`; Rhino reports corners
  at approximately ±0.8000000119 instead of ±0.8.
- Six transformed disk-face cases have a Z maximum of 4.0374999046 in Rhino,
  versus the analytic 4.0390625 (difference `0.0015626`).
- Six oblique-plane surface/disk cases differ by up to `0.0007082` in world
  corners. Independent quadratic extrema tests check all 18 curved diagnostic
  outputs, including interior stationary points and circular boundaries.
- Two small/thin inputs remain solids natively but collapse/fail in Rhino.
  Additional probes at document tolerance `1e-9`, spanning XY scales 0.01–1e6,
  found Rhino collapsing thickness `1e-7` but retaining `2e-7`. The command's
  complete cutoff policy is not established; native code retains features
  resolved by the requested tolerance.
- Six mixed valid/point selections expose Rhino's partial output/report behavior;
  native execution is atomic.

The oracle records corners, face sizes, closure, groups, source retention,
selection, current layer, and report count. Rhino's `RunScript` returns false
for a valid three-dimensional `Output=None` report but true for a planar one;
the probe therefore records whether reports exist without an explicit failure.
Raw script flags/history remain available with `inspect=true` on the Rhino
worker. Report text formatting is not an exact compatibility claim.
Corner sets do not establish identical curve seams/parameterization or B-rep
face ordering.

Native tests additionally check all six world plane presets, oblique frames,
CPlane origins at `1e100`, local coordinate reports, rational weight gauges,
trimmed versus underlying surfaces, tolerance-controlled thin solids, unchanged
sources, current layers, undo/redo, and atomic failures. Python ownership tests
exercise initialization, insertion, command, report, and record failures while
preserving pre-existing objects/groups/selection and viewport/application state.
A private GUI smoke test exported all three geometry outputs in an oblique
CPlane after undo/redo and a report-only command; 3DM corner error was below
`4.2e-15`. These command probes are untimed; no BoundingBox performance parity
is claimed.
