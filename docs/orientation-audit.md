# Orientation and document insertion audit

`orientation_audit` is a Rhino-only diagnostic, not an operation accepted by the
native oracle. Its source-only generator constructs geometry through public
RhinoCommon APIs in an owned private-Xvfb session. It separately records source
geometry, document insertion, reversal followed by replacement, and actual `Flip`
command results. It never decompiles or inspects proprietary code.

The retained [request](../tools/rhino_oracle/fixtures/orientation_audit.json) and
[observations](../tools/rhino_oracle/observations/orientation_audit.json) contain 36
cases on Rhino 8.32.26160.13001: 20 insertion/replacement cases and 16 actual command
cases. Both generic insertion and the `AddBrep` overload with kinky-face splitting
disabled are tested. Caller-owned source geometry is checked for mutation; object
identity and counts are checked after document edits. Names, group membership,
selection, complete geometric definitions, and named command events are retained.

Observed single-shell behavior:

- Closed boxes and spheres are oriented outward on insertion and replacement.
- Open B-reps preserve their input sense on insertion and accept reversed replacements.
- Meshes preserve input winding, including inward closed meshes.
- `Flip` skips closed B-reps and points but flips open B-reps, curves, and meshes.
- Preselected objects stay selected. Postselected flipped objects are deselected;
  skipped objects stay selected, even within a mixed group.

For open B-reps, all geometry except the topology's face-sense flags is identical
before and after `Flip`. Surface UVs, spatial edges, trim curves, and tolerances do
not change. This motivates the native command's independent oriented-face model.

The mesh `SolidOrientation()` diagnostic remains unchanged after raw face winding
has reversed in these captures. It must not override the retained mesh faces as
evidence. This is an observed getter inconsistency, not proof of its internal cause.

The raw response was compacted as JSON, retaining all values including the exact
IEEE-754 bit patterns of every number. Tests verify definitions and selections;
no result is rewritten to agree with a desired native outcome. This is not an
identical-source native geometry comparison or performance benchmark.

Reproduce the primary capture with:

```sh
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/orientation_audit.json --timeout 1800
python3 -m unittest tools.rhino_oracle.test_orientation
```

Document-wide normalization remains separate from [`Flip`](commands/flip.md).
Multi-shell solids, cavities, and coincident/opposed shells require separate
evidence; a volume-sign shortcut must not be inferred from a single inward box.
