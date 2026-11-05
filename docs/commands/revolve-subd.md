# Revolve SubD output

Select profiles and enter:

```text
Revolve 0,0,0 0,0,1 360 Output=SubD SegmentCount=8 DeleteInput=No
```

`SegmentCount` sets angular faces and defaults to ten. Values below four clamp
to four. Counts through 100,000 are accepted, subject to the shared SubD limit
of 100,000 control vertices and faces per result. `PointCount` applies separately
to deformable Surface output. Surface-only Deformable and SplitAtTangents choices
remain inactive while Output is SubD; global CreaseSplitting also leaves this
output unchanged.

The profile is fitted to a non-rational uniform cubic through Greville stations.
Cubic inputs omit the two adjacent endpoint handles; other degrees use every
station. Open profiles get natural ends, and closed profiles get a periodic fit.
This can change the profile substantially: Rhino's fitted result for a single
cubic Bézier can be a straight line. Use Surface output when an exact revolution
of the source is required.

Angular stations use natural interpolation for partial turns and periodic
interpolation for full turns. Negative turns start their increasing chart at
the geometric end angle. Axis poles keep adjacent handles, and shared native
control-net cleanup sews seams and forms pole fans. Regular bands precede those
fans. Source clamping and first-axis-contact handling follow [Revolve](revolve.md).

Start bare `Revolve` to pick the axis. `Output=` and `SegmentCount=` can change
before either pick, including switching back to Surface output. Escape preserves
the document and pending Redo. Outputs use the current layer, stay unselected
and leave source groups. DeleteInput controls source removal in the same undo
step. Options reset between invocations.

The [validation record](../revolve-subd-validation.json) covers 80 native command
workflows, 17 independent SDK profile fits, app replay and 3DM round trips in both
builds. Tagged control topology, oriented limit quads and history are compared
by geometric correspondence. The absolute epsilon is `1e-12`, except `2e-9` for
the fixture translated to million-unit world coordinates. Native control-vertex,
edge and display-buffer numbering are not reproduced. Continuous error bounds,
arbitrary topology and comparative performance remain unqualified.

```sh
cargo test -p viboceros-oracle --test revolve_subd --test revolve_subd_archive
cargo test -p viboceros revolve_subd
python3 -m unittest tools.rhino_oracle.test_revolve_subd
```

Licensed captures use private Xvfb sessions through
`tools/rhino_oracle/run_headless.sh`.
