# Soft SubD crease discovery

[Rhino oracle](../oracle.md) · [Editing evidence](editing-evidence.md)

The [32 saved SubDCrease workflows](../../tools/rhino_oracle/observations/soft_crease_discovery.json)
were captured from Rhino 8.32.26160.13001 on a private Xvfb display and settings
scheme. They preserve fixed inputs, command events, source and result 3DM files,
attributes, selection, Undo/Redo, and the original public producer source bytes.

The capture covers constant weights from 0 through 100, repeated weight input,
ConstantWeight, CreaseAll, cancellation before and after a weight, hard/corner
sources, boundary edges, whole-object and vertex selections, and chain, branch
and loop selections. Cancel after a pending weight restores the original model.
Zero/default edits create no model history record; Undo reaches the setup witness.

Boundary-only, whole-object and vertex-only inputs cancel in the edge getter;
their unknown numeric input remains in the transcript. Typed coordinates used
in the label experiments did not enter a vertex-weight getter. Those recipes
therefore do not establish per-vertex weight interaction.

This is discovery evidence for subsequent implementation. It does not qualify a
Viboceros SubDCrease command, variable-weight chains, mouse label input, numerical
agreement or performance. The existing Crease and RemoveCrease qualification is
separate. Replay the capture integrity checks without starting Rhino:

```sh
python3 -m unittest tools.rhino_oracle.test_soft_crease
```

The generator refuses to overwrite its observations. Source snapshots retain
the exact producer version used by the capture when shared helpers change.
