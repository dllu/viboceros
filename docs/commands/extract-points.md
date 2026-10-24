# ExtractPt

`ExtractPt` copies defining locations into individual point objects or one point
cloud. Preselect sources to execute immediately, or start the command, select
sources or displayed control/edit grips, and press Enter. While selecting, choose `Output` and `OutputLayer`,
either by entering their names or complete options. Escape cancels; Enter with
no eligible sources exits. Option edits remain local to the invocation.

```text
ExtractPt Output=Points OutputLayer=Input
ExtractPt Output=PointCloud OutputLayer=Current
```

Sources contribute curve defining points, NURBS surface control nets, B-rep
face support controls, raw mesh vertices, SubD control vertices, or point-cloud locations.
Raw mesh locations include unused and coincident vertices. Closed curve seams
and periodic control rings retain their existing Rhino ordering. Source order
follows selection actions. Point objects and empty meshes contribute nothing;
the interactive filter excludes them and block instances.

Selected grips contribute their current displayed locations, including SubD
edit points. They replace whole-object extraction for their owner. Other whole
objects contribute first; grips follow in document order and ascending index,
independently of their selection order. Hidden grips are excluded. Point
selectors such as `SelPt`, `InvertPt`, and U/V navigation also work during selection.

`Output=Points` is the default. Results use each input's layer and attributes
and groups with `OutputLayer=Input`, or fresh attributes on the current layer with
`OutputLayer=Current`. `Output=PointCloud` combines all locations into one cloud;
input-layer attributes come from the first selected source when it contributes
locations, otherwise the cloud uses fresh current-layer attributes. When grips
contribute, the first grip owner's attributes and groups take precedence.
Results are selected without expanding their groups. Sources remain unchanged.
Point output clears grip selection. Cloud output from whole objects or grips
retains it; command-first Undo/Redo releases those grips, while preselected cloud
history retains them. Component input releases command-first grip picks immediately.
An owner selected before starting remains selected when extracting its grip
points. Command-first history releases that owner; cloud history releases
both owner and grip picks for command-first extraction.
Creation is one Undo step,
and Redo restores the same results. Unsupported typed sources, invalid options,
and the cumulative one-million-point limit reject the command atomically.

Extraction borrows source geometry and stages locations before model edits.
Individual point attributes are copied when admitted; point-cloud output keeps
one attribute record instead of copying it for every location.

The interactive regression replays 44 saved Rhino curve cases with both
preselection and command-first selection and both output types (176 workflows),
checking locations at `1e-8`, unchanged sources and Undo/Redo. These cover seam,
closure, rational and nested-curve cases. The same captures also qualify the
existing backend extraction path. A separate [grip qualification](../extract-points-qualification.json)
replays 63 native scenes through the backend and their corresponding interactive
selection paths (126 workflows), comparing geometry, attributes, ordered groups, grip
visibility/selection, source storage and Undo/Redo at `1e-7`. Thirteen additional
captures retain native macro-option discovery; command-first captures apply
both output options before selection.

[Component extraction](extract-point-components.md) supports B-rep faces and
edges, and mesh/SubD faces, edges and topology vertices, including smooth SubD
edge control points. Smooth SubD whole-object limit locations and `SubCrv`
remain unimplemented.

`OutputLayer=TargetObject` remains unavailable. The installed Rhino
8.32.26160.13001 rejects that value, despite its inclusion in the command
reference; the [native menu capture](../../tools/rhino_oracle/observations/extract_points_target_menu.json)
retains the command history. These diagnostic captures do not qualify target-layer output.

See [Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractpt.htm)
and [point-cloud commands](point-cloud.md).
