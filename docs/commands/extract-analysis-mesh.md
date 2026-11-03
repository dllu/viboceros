# Extract analysis mesh

[Analysis commands](analysis-index.md) · [Draft analysis](draft-angle-analysis.md) · [Curvature analysis](curvature-analysis.md)

`ExtractAnalysisMesh` copies analysis meshes into independent, editable mesh
objects. Select objects first or pick them at the prompt. Surfaces, polysurfaces,
typed extrusions and SubDs produce one combined mesh per source. Blocks produce
one mesh per meshable placed leaf, using that leaf's attributes. Ordinary mesh,
curve and point inputs produce no output.

An analysis mesh is generated if needed. Active draft/curvature analysis colors
become mesh vertex colors; after analysis is turned off, extraction produces an
uncolored mesh. The source and its analysis membership remain intact. Later
analysis-setting changes do not recolor extracted meshes.

Outputs retain source names, layers, object colors, wire density and attribute
user text. Group membership and geometry user text are omitted. Preselection
remains selected; command-first picks are released. Outputs start unselected.
The whole extraction is one model edit for Undo/Redo; an empty result preserves
model history and redo.

Display and extraction share immutable analysis tessellations. Copies retain
stored normals and separate vertices where normal or color sectors differ;
quad faces remain quads when their corner channels agree. Draft display uses
analytic surface normals from this same representation. NURBS curvature samples
are evaluated only when curvature is requested. Curvature samples that
are unavailable retain the display's gray color.

[53 native recipes](../../tools/rhino_oracle/fixtures/extract_analysis_mesh.json)
and their [frozen observation](../../tools/rhino_oracle/observations/extract_analysis_mesh.json.gz)
qualify selection, attributes, color presence, generation without prior analysis,
blocks, repeat extraction and Undo/Redo. Source geometry CRCs remain unchanged.
App replays exercise both preselection and viewport picks. Renderer checks compare
submitted colors with extracted corner colors in all display modes. Tessellation
counts and sampling locations are not identical to Rhino's analysis mesher;
[analysis mesh density controls](analysis-mesh.md) apply to both display and extraction.

```sh
cargo test -p viboceros-command --lib extract_analysis_mesh
cargo test -p viboceros --bin viboceros extract_analysis_mesh
cargo test -p viboceros --bin viboceros extracted_analysis_mesh
python3 -m unittest tools.rhino_oracle.test_extract_analysis_mesh
```

Reference: [Rhino ExtractAnalysisMesh](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractanalysismesh.htm).
