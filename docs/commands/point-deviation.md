# PointDeviation

`PointDeviation` measures objects against the nearest locations on target
curves, surfaces, polysurfaces or meshes. Select measured objects first, or
start the command and select them, then press Enter. Pick the targets and press
Enter again. Escape cancels the picker and restores the original selection.
Point objects and point clouds contribute their stored locations. Mesh sources
contribute all stored vertices, including unused vertices. Block instances are
excluded.

```text
Line 0,0,0 10,0,0
Point 2,1,0
SelPt
PointDeviation Ignore=10
```

Pick the line and press Enter. The Point Deviation panel shows count, mean,
median, population standard deviation, minimum and maximum distances in model
units. Colored lines extend from each closest target location toward the
measured point. `Hair scale` exaggerates that distance for display; it does not
change the measurements.

The panel controls good/bad colors and thresholds, ignore distance, proximity
angle, hair visibility and sample resolution. Distances at or below Good use
the good color; distances at or above Bad use the bad color. Intermediate
distances interpolate the colors. Require `0 ≤ Good ≤ Bad ≤ Ignore`. Points
beyond Ignore are excluded from statistics and display. IgnoreBad and proximity
angle filter the display only. At 180 degrees every measured direction passes;
at smaller angles, undefined normals hide the corresponding hair. Curve
proximity measures deviation from the normal plane; surface/mesh proximity
measures deviation from either normal direction.

Curve sources use equal arc-length stations, including open endpoints and a
single closed seam. Polyline vertices are additionally retained. Surface and
B-rep sources use vertices from the existing display tessellator; their sample
sets can differ from Rhino's analysis mesh. CurveDivisions defaults to 100 and
SurfaceSamples defaults to 8 per knot span. The initial display thresholds are
Good=0.01, Bad=0.1, Ignore=1 in model units; these defaults are local policy.

Geometry edits, Undo and Redo update the analysis automatically. Unchecking
automatic updates freezes the sampled geometry until Apply is pressed. Display
options and the statistics ignore threshold still apply immediately. Repeated
frames reuse the stored samples and statistics. Deleted source/target objects
produce an error without displaying stale results; Undo can restore them.
Start opens a new picker, End releases the analysis, and closing the window
hides the panel. `PointDeviation Visible=Yes` reopens it.

`Make hair permanent` adds the displayed nonzero segments to layers named
`Point Test r,g,b` in one Undo/Redo edit, retaining the analysis. It uses the
current exaggeration and colors. Short segments rejected by the existing line
constructor or nonrepresentable exaggerated endpoints reject atomically.
Measurements and display options alone do not consume model history or Redo.

Scripts can specify object IDs explicitly:

```text
PointDeviation Sources=1,2 Targets=3,4 Ignore=10 ProximityAngle=180
PointDeviation Good=0.05 Bad=0.5 HairScale=5
PointDeviation Automatic=No
PointDeviation Apply
PointDeviation MakePermanent
PointDeviation End
```

Use IDs reported by your own document. Sources defaults to selected objects
other than the explicit targets. The public Rust `analyze` function also accepts
an array of `(source ID, position)` pairs and borrowed target geometry. It
preserves duplicate point occurrences and uses exact point-distance comparisons
to choose between target candidates. Mesh targets prepare the existing spatial
index once for the batch. Curve and trimmed-face targets reuse the established
closest-point queries. No additional geometric solver or native dependency is
introduced. Sampling is capped at one million points and four million point/
target pairs; curve divisions range from 1 to 10000 and surface samples from
1 to 128 per span.

The [retained public SDK capture](../../tools/rhino_oracle/observations/point_deviation_rhino_20261016.json)
contains 14 recipes and 65 query points covering analytic, rational and composite
curves, a surface, box and mesh B-reps, mesh faces, multiple targets, duplicate
points and a remote line. All 65 nearest locations pass exact squared-distance
admission at `1e-6`; distances and derived statistics also agree at `1e-6`.
The largest paired closest-point distance is `5.97e-10`.

This capture uses public SDK closest-point methods. It does not capture Rhino's
PointDeviation panel, source sampling, defaults, proximity/color policies or
permanent-layer policy. Native command and panel parity, ambiguous nearest
locations, general SubD input and performance remain unqualified. The local
population statistics are calculated independently from the captured SDK
distances. See [McNeel's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/pointdeviation.htm),
the [comparison](../../tools/rhino_oracle/observations/point_deviation_comparison_20261016.json)
and [validation record](../point-deviation-validation.json).
