# Typed extrusion creators

[Geometry guides](geometry/README.md) · [Extrusion storage and interchange](extrusions.md)

`Box`, `Cylinder` and eligible `ExtrudeCrv` results now retain native extrusion
geometry alongside its canonical B-rep view. They can be saved to 3DM as extrusion
objects and converted whole with [ToSubD](commands/to-subd.md).

- Box creates a closed rectangular extrusion in the active construction plane.
- Cylinder creates a circular extrusion. Its default is `Solid=Yes`;
  `Solid=No` leaves the wall open. Negative heights and BothSides retain the
  requested path direction, with a path interval starting at zero.
- ExtrudeCrv uses typed extrusions for planar profiles swept perpendicular to
  their plane, including lines and kinked profiles. Oblique and nonplanar
  inputs use explicit B-reps. Closed planar profiles receive caps with
  `Solid=Yes`; open profiles remain open.

These rules follow the public [ExtrudeCrv documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrv.htm)
and fixed native observations. Construction reuses public OpenNURBS extrusion
factories. General sweeps continue to use usd-brep/SMLib when enabled.

Closed profiles reverse when the path direction requires it. Capped results
retain the resulting signed profile interval; uncapped command results restore
the original interval. The canonical B-rep supplies trimmed faces and solid
operations. `Geometry::nurbs_surface_representation` provides an untrimmed
surface view for read-only comparisons; extrusion caps and B-rep trims are
excluded from that view.

The [qualification record](creator-extrusion-qualification.json) covers 42 native
creator workflows: 39 typed extrusions and three explicit B-reps. It checks
surface controls, visible bounds, source preservation, grouped undo/redo and
3DM type retention. Forty eligible outputs also match native SubD controls,
tags, vertex-limit stations and corresponding sampled facets within `1e-6`.
The [default-build record](creator-extrusion-portable-qualification.json) checks
the same 42 workflows. Explicit portable sweeps reuse the existing kink-face
splitter to retain separate wall faces.

Box profiles are compared geometrically under cyclic seam changes. Exact Box
parameter origins and face ordering remain unqualified. The other surface
comparisons retain full grids and knot intervals, with axis reversals and
transposition allowed for explicit oblique walls. Native attributes, pointer
getters, arbitrary profiles, continuous surface error and performance are
outside this record's scope. A failed initial Cylinder macro is retained as
an excluded diagnostic; corrected captures set Solid at the base prompt.

```sh
cargo test -p viboceros-command --release --features native-smlib \
  --test creator_extrusions_native -- --nocapture > /tmp/creator-extrusions.log
python3 -m tools.rhino_oracle.report_creator_extrusions /tmp/creator-extrusions.log
python3 -m unittest tools.rhino_oracle.test_creator_extrusions
```

The [integration validation record](creator-integration-validation.json) lists
the command/application checks and four reproduced baseline UI failures that
remain outside this creator change.

The creator command and regression code live in focused Cylinder and ExtrudeCrv
modules. The README remains a short setup guide; qualification details belong
here and in the machine-readable record.
