# PackSubDFaces

```text
PackSubDFaces
```

Select whole SubDs, then run the command, or run it first, pick SubDs and press
Enter. It recomputes rectangular quad groups from current topology, vertex tags
and hard creases. Non-quads have individual pack IDs. Stored packs are replaced;
control points, component identities, topology, tags and soft sharpness stay intact.

Object IDs, attributes and groups survive. Replaced objects move to the end of
document order in selection order. Preselection remains selected, including
unrelated selected objects; prompted picks are cleared. Cancellation preserves
geometry and history. All replacements form one undoable edit, including repeated
packing. Geometry is staged before mutation, with a 64 MiB output archive budget.

The pack metadata survives 3DM save/load and subdivision and is used by
[packed ToNURBS conversion](to-nurbs-subd.md). The Rust grouping algorithm is
shared with that converter. Texture rectangles use our deterministic grid atlas
and a distinct algorithm UUID; Rhino's texture rectangle allocation is not
reproduced. The small OpenNURBS metadata extension is compiled against the pinned
kernel source, leaving the upstream checkout unchanged.

Qualification includes 17 frozen actual-command workflows (33 steps), with
mixed selections, both selection orders, cancellation, repeated commands and
undo/redo. Imported control geometry and per-face pack IDs match the successful
captures. Independent SDK captures cover 36 source profiles, including irregular
and wrapped nets. Rust tests also check stale crease repair, archive round trips
and retained packs after subdivision; UI tests run without opening windows.

ToNURBS edge picks and its `ShowFacePacks`/`ForceUpdateFacePacks` prompt
actions remain separate work. See the
[Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/packsubdfaces.htm).
