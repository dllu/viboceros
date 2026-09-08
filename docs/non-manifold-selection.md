# Non-manifold selection oracle

All four cases agree exactly in the [retained comparison](non-manifold-selection-measurement.json),
collected on 2026-09-08 with the licensed Rhino 8 Wine/FEX installation in a
private Xvfb session.

`tools/rhino_oracle/fixtures/non_manifold_selection.json` exercises the actual
`SelNonManifold` command in Viboceros and Rhino, not only their topology APIs.
Each case creates an open triangle, a closed tetrahedron, and a tetrahedron
with an extra face sharing one of its edges. Cases cover both triangle meshes
and trimmed B-reps constructed from those meshes.

Without preselection, only the non-manifold object (index 2) should be selected.
Separate additive cases start with the open object selected and expect indices
0 and 2. Separating these cases prevents preselection from masking an incorrect
selection of an open manifold object.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/non_manifold_selection.json --timeout 180
```

The Rhino worker saves/restores prior selection, disposes constructed geometry,
and deletes only its temporary objects, including after a command failure.
Python tests verify success/failure cleanup and reject non-boolean mode flags.
Partial-construction tests inject failed mesh additions, failed B-rep conversion,
and failed B-rep additions on the second object. They check disposal of created
geometry, deletion of only the first successfully added object, restoration of
prior selection, and cancellation without running the selection command.
The native probe checks that geometry and undo/redo labels remain unchanged.
These probes are untimed; zero timing fields are not performance measurements.

This fixture does not establish arbitrary topology parity. Hidden/locked
exclusions, object attributes, and invalid command arguments have native command
tests but are not exercised in this live fixture.
