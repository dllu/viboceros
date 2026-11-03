# Analysis meshes

[Draft angle analysis](draft-angle-analysis.md) and
[curvature analysis](curvature-analysis.md) share document analysis mesh settings.
Both panels provide NURBS density, Simple planes, SubD subdivision/adaptive
controls and Restore defaults. Changes refresh display and [ExtractAnalysisMesh](extract-analysis-mesh.md),
including placed block leaves, without editing source geometry, selection,
model history or an active point getter. Ordinary viewport meshing is separate.

NURBS/B-rep density defaults to `0.8` and accepts `0..1`. Positive values use
[geometric Bézier error estimates](../nurbs-meshing.md); the initial grid has at
least sixteen cells per face before trimming. Bilinear faces keep that grid.
Simple planes skips interior refinement on planar surfaces. Each face can use
different U/V counts, preserving sparse rows along straight extrusion directions.
Trim boundaries share edge sampling; the conforming fallback retains
face provenance, one-sided crease normals and per-face interior grids.

SubDs default to adaptive level 4. The slider exposes levels 1–5, giving an
isolated quad a 2×2 through 32×32 grid. Adaptive mode reduces subdivision when
control-face count times `4^level` exceeds 512,000, down to level 1. The API and
console accept levels 0–6; level 0 needs one subdivision for nonquad fragments.
Imported values outside the slider range remain intact until edited.

These Viboceros console bindings expose panel actions:

```text
DraftAngleAnalysis NurbsDensity=0.5
CurvatureAnalysis NurbsDensity=0.8 SubDLevel=4 SubDAdaptive=Yes
CurvatureAnalysis SimplePlanes=Yes
CurvatureAnalysis MeshDefaults=Yes
```

Settings are stored in native 3DM analysis meshing parameters. Open applies native
legacy profile migration; Save/SaveAs/Export retain detailed controls, including
ones without UI bindings. Import keeps the destination policy. Restore defaults
clears retained controls as well as the sliders and plane setting.
BlockEdit save/cancel and model Undo/Redo retain the current display settings.
Analysis membership and color ranges remain session settings. Cache keys include
only the parameters relevant to each geometry family, so changing NURBS density
does not discard SubD geometry, or vice versa.

Native Rhino 8.32 archive reads and rewrites cover five NURBS densities and six
SubD configurations. Another nineteen profiles qualify document load/save
migration: stored default geometry settings become default analysis settings;
the legacy analysis profile with zero relative tolerance becomes the fast profile
(`0.65`, simplified planes). Matching includes SubD level and adaptive mode, and
ignores curvature, texture and custom flags. Analysis texture ranges become
`[0,1]`. Custom zero-density profiles and tiny positive densities survive.
Raw archive APIs preserve the original profile.

Mesh and native vertex samples are tested independently. Identical Rhino
analysis tessellation, pixels and trimmed-face error bounds remain unqualified.
Detailed meshing controls beyond density and simplified planes are retained but
not fully applied to meshing. Adaptive NURBS/B-rep meshing enforces a 1,048,576
vertex limit and a refinement-work budget; SubD meshing limits output to one
million faces. Unavailable analysis objects are reported by the curvature panel.

```sh
cargo test -p viboceros-geometry --lib analysis_nurbs_density
cargo test -p viboceros-command --lib analysis_nurbs_density
cargo test -p viboceros --bin viboceros analysis
python3 -m unittest tools.rhino_oracle.test_analysis_nurbs_density
python3 -m unittest tools.rhino_oracle.test_analysis_profile
```

Reference: [Rhino analysis mesh options](https://docs.mcneel.com/rhino/8/help/en-us/information/analysismeshoptions.htm).
