# Conversion to SubD

[Architecture](architecture.md) · [Compatibility evidence](oracle/README.md)

The [ToSubD command](commands/to-subd.md) exposes mesh, surface and typed
extrusion conversion through scripted input and application prompts, including
B-rep face selection. [Extrusion representation and file interchange](extrusions.md)
are available, including [typed creator integration](creator-extrusions.md).

- `SubD::try_from_mesh` accepts independent interpolation, corner and crease
  options through `SubDFromMeshOptions`.
- `SubD::try_from_surface` accepts `ControlPoints` or `Location` through
  `SubDFromSurfaceMethod`, plus boundary corner handling.

Mesh conversion uses public OpenNURBS topology construction. Location mode
reuses the sparse vertex-limit interpolation solver. Surface control nets reuse
control-polygon cleanup, including periodic windows, closed seams and pole
triangles. Surface Location fitting solves separable sparse systems at Greville
stations, with natural or periodic cubic rows, singular boundary rows and G1
crease anchors. Input geometry remains unchanged on success or failure.

The [qualification record](tosubd-kernel-qualification.json) covers 124 kernel
comparisons: 16 meshes, 45 surface control nets and 63 surface Location fits.
These include one rejected collapsed control net. Constructive cases compare
control topology, tags, limit stations and corresponding density-two facets
within `1e-6`. Cases include rational and nonuniform surfaces, unclamped and
periodic axes, sphere poles, hemispheres, tori, and degree-elevated cones.
A further [62 comparisons](tosubd-extrusion-qualification.json) cover 54 whole
extrusions and eight direct surface-SDK cases. They include transposition,
one-ended caps, complete-profile joining, and linear/quadratic crease behavior.
Closed complete profiles receive caps when both ends are capped; one-ended
inputs retain their walls, matching the recorded native behavior. Multiple
profiles follow the native joined-profile interpretation, which can be open.

Continuous surface error, arbitrary topology and comparative speed remain
unqualified.

The [document workflow record](tosubd-command-qualification.json) covers 201
command and 200 application prompt replays from 206 corrected native workflows;
five invalid profile diagnostics are excluded. They check output geometry,
attributes, selection, identity, groups, cancellation and grouped Undo/Redo.
An additional 43 initial diagnostic workflows retain the original incorrect
option timing and remain excluded. Surface inputs reuse recorded NURBS tables;
ordinary boxes retain full topology and extrusions load native 3DM sources.
Trimmed source topology and cross-command native preference sequences remain
unqualified. Local tests cover remembered options, stale sources and face picking
after command startup.

To rerun the kernel comparisons and report:

```sh
cargo test -p viboceros-command --release --test tosubd_kernel_native \
  -- --nocapture > /tmp/tosubd-kernel.log
python3 -m tools.rhino_oracle.report_tosubd /tmp/tosubd-kernel.log
python3 -m unittest tools.rhino_oracle.test_tosubd
```

The report rejects missing, duplicate, malformed and out-of-tolerance results.
Capture recipes and archived producer sources are immutable; the initial
incorrect option-phase experiments are excluded from kernel totals.

To rerun document and prompt qualification:

```sh
cargo test -p viboceros-command --release --test tosubd_native --test tosubd \
  -- --nocapture > /tmp/tosubd-command.log
cargo test --release -p viboceros --bin viboceros app::to_subd::tests \
  -- --nocapture > /tmp/tosubd-getter.log
python3 -m tools.rhino_oracle.report_tosubd_commands \
  /tmp/tosubd-command.log /tmp/tosubd-getter.log
python3 -m unittest tools.rhino_oracle.test_tosubd_commands
```
