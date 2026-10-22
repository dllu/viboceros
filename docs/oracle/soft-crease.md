# Soft SubD crease observations

[Rhino oracle](../oracle.md) · [Editing evidence](editing-evidence.md)

The initial [32 SubDCrease workflows](../../tools/rhino_oracle/observations/soft_crease_discovery.json)
and subsequent captures retain 178 workflows from Rhino 8.32.26160.13001 on private
Xvfb displays and settings schemes. They preserve fixed inputs, command events, source and result 3DM files,
attributes, selection, Undo/Redo, and the original public producer source bytes.
Another [48 chain-editing workflows](soft-crease-chains.md) bring the retained
corpus to 226 workflows, including the earlier diagnostic families.

The capture covers constant weights from 0 through 100, repeated weight input,
ConstantWeight, CreaseAll, cancellation before and after a weight, hard/corner
sources, boundary edges, whole-object and vertex selections, and chain, branch
and loop selections. Cancel after a pending weight restores the original model.
Zero/default edits create no model history record; Undo reaches the setup witness.

Boundary-only, whole-object and vertex-only inputs cancel in the edge getter;
their unknown numeric input remains in the transcript. Typed coordinates used
in the label experiments did not enter a vertex-weight getter. Those recipes
therefore do not establish per-vertex weight interaction.

The [geometry API qualification](../subd-soft-crease-qualification.json) now
compares 133 workflows, including separate live-result and archive-readback
checks. The numeric-label family uses the shared exact decimal formatter;
earlier attempts rounded neighboring doubles before delivering them to Rhino.
Complete mouse traces confirm ten vertex-weight getter workflows. A further
click after changing a global weight missed the moved label; its following
numeric weight was global. Failed and incomplete plans stay in three diagnostic
families, outside the qualified comparison set.

Branch and disconnected selections expose native CreaseThisChain and
ConstantWeightChain options in the vertex-weight getter. A junction can belong
to several chains; clicking it edits the selected chain rather than every
incident edge. Closed-loop observations retain their intermediate zero anchor.

The application SubDCrease getter, profile editor and comparative performance
remain subsequent work. The existing Crease and RemoveCrease qualification is
separate. Replay the checks without starting Rhino:

```sh
python3 -m unittest tools.rhino_oracle.test_soft_crease
```

The generator refuses to overwrite its observations. Source snapshots retain
the exact producer version used by the capture when shared helpers change.
