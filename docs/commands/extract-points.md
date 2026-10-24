# ExtractPt

`ExtractPt` copies defining locations into individual point objects or one point
cloud. Preselect sources to execute immediately, or start the command, select
sources, and press Enter. While selecting, choose `Output` and `OutputLayer`,
either by entering their names or complete options. Escape cancels; Enter with
no eligible sources exits. Option edits remain local to the invocation.

```text
ExtractPt Output=Points OutputLayer=Input
ExtractPt Output=PointCloud OutputLayer=Current
```

Sources contribute curve defining points, NURBS surface control nets, B-rep
vertices, raw mesh vertices, SubD control vertices, or point-cloud locations.
Raw mesh locations include unused and coincident vertices. Closed curve seams
and periodic control rings retain their existing Rhino ordering. Source order
follows selection actions. Point objects and empty meshes contribute nothing;
the interactive filter excludes them and block instances.

`Output=Points` is the default. Results use each input's layer and attributes
with `OutputLayer=Input`, or fresh attributes on the current layer with
`OutputLayer=Current`. `Output=PointCloud` combines all locations into one cloud;
input-layer attributes come from the first selected source when it contributes
locations, otherwise the cloud uses fresh current-layer attributes. Results
replace the selection; sources remain unchanged. Creation is one Undo step,
and Redo restores the same results. Unsupported typed sources, invalid options,
and the cumulative one-million-point limit reject the command atomically.

Extraction borrows source geometry and stages locations before model edits.
Individual point attributes are copied when admitted; point-cloud output keeps
one attribute record instead of copying it for every location.

The interactive regression replays 44 saved Rhino curve cases with both
preselection and command-first selection and both output types (176 workflows),
checking locations at `1e-8`, unchanged sources and Undo/Redo. These cover seam,
closure, rational and nested-curve cases. The same captures also qualify the
existing backend extraction path. Whole-object extraction is supported here;
selected grips, mesh/SubD components, trimmed face controls, smooth SubD limit
locations, `SubCrv` and `OutputLayer=TargetObject` remain unimplemented.

See [Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractpt.htm)
and [point-cloud commands](point-cloud.md).
