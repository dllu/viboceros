# ExtrudeCrv SubD output

Select curves and enter a distance or two direction points:

```text
ExtrudeCrv 3 Output=SubD BothSides=Yes DeleteInput=No
ExtrudeCrv 0,0,0 1,2,3 Output=SubD
```

Direction, BothSides and DeleteInput follow the ordinary
[straight extrusion](surfaces.md) rules. SubD output fits the swept profile to
non-rational cubic limit stations. It can change the source shape; choose the
default `Output=Surface` when exact extrusion geometry is required.

The shared surface-to-SubD fitter uses Greville samples, natural open ends and
periodic closed seams. Natural cubic pieces omit adjacent handles at their ends,
including fully multiple joins, while retaining crease stations. Curved cubic
Bézier profiles use their interior stations; their behavior differs from
[SubD Revolve](revolve-subd.md). Planar oblique profiles follow the public
OpenNURBS support-plane orientation.

`Solid` applies to Surface output. Switching to SubD leaves both ends uncapped,
including for a closed planar profile with Solid previously enabled.
`UseExtrusions` does not change SubD output. Existing conversion budgets apply,
including 100,000 control vertices and faces per result.

Start bare `ExtrudeCrv` to pick the direction base and target. `Output=Surface|SubD`
can change before either point, along with BothSides, DeleteInput and the stored
Solid option. Escape preserves the model and pending Redo. Options reset for
each invocation. Outputs use fresh attributes on the current layer, remain
unselected and ungrouped, and share one undo step with source deletion.

The [validation record](../extrude-curve-subd-validation.json) covers 62 native
command workflows, 10 public SDK surface fits, direction-point app replay and
3DM round trips in both builds. Tagged control topology, oriented limit quads
and history use absolute epsilon `1e-12`. Native component IDs, face insertion
order and display-buffer numbering are not reproduced. General continuous
approximation bounds and comparative performance remain unqualified.

```sh
cargo test -p viboceros-oracle --test extrude_curve_subd --test extrude_curve_subd_archive
cargo test -p viboceros extrude_curve_subd
python3 -m unittest tools.rhino_oracle.test_extrude_curve_subd
```

Native captures use private Xvfb sessions through
`tools/rhino_oracle/run_headless.sh`.
Reference: [Rhino ExtrudeCrv](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrv.htm).
