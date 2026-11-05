# ConvertExtrusion

```text
ConvertExtrusion
ConvertExtrusion DeleteInput=Yes
ConvertExtrusion DeleteInput=No
```

Convert lightweight extrusion objects to explicit surface or polysurface B-reps.
Start with selected extrusions, or start the command and pick them. Enter accepts
the objects; a separate confirmation offers `DeleteInput=Yes|No`. Enter then
converts them. Escape cancels without editing geometry or consuming pending Redo.

`DeleteInput=Yes` is the default for each invocation. It replaces geometry while
retaining source UUIDs. `No` keeps the extrusion and creates an unselected B-rep
copy. Object attributes, layer, color, attribute user text and groups follow the
source. Geometry user text is absent from the converted result, matching Rhino.

Persistent preselection, including unsupported grouped peers, remains selected.
Command-first picks are released and stay released through Undo/Redo. One Undo
restores original extrusion geometry and shared storage; Redo restores conversion.
Preselected batches follow document order; prompted batches follow pick order.
The batch commit validates all operands once and retains persistent table positions.

Runtime cap planes use tight outer-trim bounds with the public OpenNURBS cap
padding policy. Public OpenNURBS' file-I/O Bézier bound uses control hulls, which
can give different cap domains on sheared profiles. The independent Rust bound
solver reproduces runtime domains without fitting the extrusion or its edges.
`Brep::try_extrusion_brep_form` exposes this query; retained extrusion archives
and their existing [file-interchange view](../extrusions.md) remain unchanged.

## Qualification

Private Xvfb captures from Rhino 8 cover 65 workflows: five input/default probes
and 60 conversions over eleven shared 3DM sources. These include open/capped
rectangles and circles, negative heights, kinked profiles, arcs, miters, shear,
pipes, grouped peers, paired objects and reversed pick order. Backend, application
and release Python replays compare complete surfaces, trims, topology, attributes,
selection, object identity roles, runtime order and Undo/Redo within `1e-12`.

```sh
cargo test -p viboceros-geometry --test extrusion_brep
cargo test -p viboceros-command --lib convert_extrusion
cargo test -p viboceros-oracle --test convert_extrusion
cargo test -p viboceros --bin viboceros convert_extrusion
python3 -m unittest tools.rhino_oracle.test_convert_extrusion
python3 -m tools.rhino_oracle.convert_extrusion_replay
```

Sources: [Rhino ConvertExtrusion](https://docs.mcneel.com/rhino/8/help/en-us/options/general.htm#ConvertExtrusion)
and public [OpenNURBS cap construction](https://github.com/mcneel/opennurbs/blob/8.x/opennurbs_beam.cpp).
Arbitrary profiles and comparative performance remain outside this capture set.
