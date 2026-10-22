# Mesh and surface conversion to SubD

[Architecture](architecture.md) · [Compatibility evidence](oracle/README.md)

The geometry API now provides the conversion foundation for `ToSubD`.
The command and interactive workflow are still being implemented. Native
[extrusion representation and file interchange](extrusions.md) are available;
ToSubD extrusion conversion remains in progress.

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
Continuous surface error, arbitrary topology and comparative speed remain
unqualified.

The saved corpus contains 139 corrected public command workflows and 43 initial
diagnostic workflows. It also retains B-rep face selection, extrusion conversion,
attributes, cancellation and Undo/Redo observations for subsequent integration.
Those document workflows have not yet been qualified against Viboceros.

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
