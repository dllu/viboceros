# Analysis meshes

[Draft angle analysis](draft-angle-analysis.md) and
[curvature analysis](curvature-analysis.md) share the document's SubD analysis
mesh settings. Each panel provides subdivision level, adaptive subdivision and
Restore defaults controls. Changes refresh analysis faces and
[ExtractAnalysisMesh](extract-analysis-mesh.md), including placed block leaves,
without changing control geometry, selection, model history or an active point
getter. Ordinary viewport meshing keeps its own settings.

The default is adaptive level 4. The slider exposes levels 1–5: an isolated quad
produces a 2×2, 4×4, 8×8, 16×16 or 32×32 grid. Adaptive mode reduces the level
when control-face count times `4^level` exceeds 512,000, down to level 1.
Disabling adaptive mode keeps the requested level. Meshing still enforces the
implementation's one-million-face allocation limit.

Both analysis commands expose these panel settings as Viboceros console bindings:

```text
DraftAngleAnalysis SubDLevel=4 SubDAdaptive=Yes
CurvatureAnalysis SubDLevel=3 SubDAdaptive=No
```

The API and console accept levels 0–6. Absolute level 0 samples an unsubdivided
quad net; nonquad faces require at least one subdivision for quad analysis
fragments. Imported levels outside the slider range remain intact until edited.
Open/Save/SaveAs/Export retain level and adaptive mode in native 3DM analysis
meshing settings. Import keeps the destination document's policy. Analysis
membership and color ranges remain session settings.

Frozen Rhino 8.32 observations cover defaults, adaptive thresholds, analysis
extraction at the default level, and native headless reads of files written by
Viboceros. Mesh face counts agree across five slider levels for a quad, box,
triangle and pentagon. Separate public `Subdivide`/`SurfacePoint` observations
qualify the sampled limit geometry to `1e-10` in those cases. Rhino's display
mesher can use different sample positions and duplicate seam vertices; identical
display tessellation, colors and pixels are not qualified. NURBS/B-rep density
and detailed meshing controls remain unimplemented.

```sh
cargo test -p viboceros-geometry --lib analysis_subd_density
cargo test -p viboceros-command --lib analysis_subd_density
cargo test -p viboceros --bin viboceros analysis_subd_density
python3 -m unittest tools.rhino_oracle.test_analysis_mesh_parameters
```

References: [Rhino analysis mesh options](https://docs.mcneel.com/rhino/8/help/en-us/information/analysismeshoptions.htm),
[public SubD display parameters](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.subddisplayparameters/).
