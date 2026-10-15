# BlendEdge

Build with `--features native-smlib`. `BlendEdge` uses usd-brep's native selected-edge executive with a quintic curvature (G2) cross-section and unit blend scale. It retains editable B-rep surfaces, trims and topology.

Start `BlendEdge`, select edges and press Enter. Edge-component preselection, click/window selection and ambiguity choices use the same picker as FilletEdge and ChamferEdge. Enter `Radius` followed by a value, or `Radius=0.5`; interactive values accept units such as `2mm`. Change `RailType=RollingBall|DistBetweenRails` while picking. Errors retain the getter and picks for retry; Esc/Cancel releases them.

```text
BlendEdge Radius=1 RailType=RollingBall object-id 0,2
BlendEdge 1 object-id 0 second-object-id 3,4
BlendEdge Radius=1 RailType=DistBetweenRails All=Yes
```

A finite positive value applies to the entire selected edge set. Defaults are `Radius=1` model unit and `RailType=RollingBall`. RollingBall locates rails using the radius of an inscribed rolling ball. DistBetweenRails measures their separation. These values locate the rails; a G2 blend does not have a constant circular radius.

Inputs currently require closed manifold B-reps. An owned native import preserves original edge indices; repeated indices are removed. Modeling is localized before import/export. Every result stages before document edits. Identity, attributes, group membership and one Undo/Redo transaction cover the batch; a later source failure preserves prior geometry and Redo. Stale source snapshots and changed tolerances are rejected.

On a right-angle straight edge the rolling mode has setback `r`; the width mode has setback `d/sqrt(2)`. For unit blend scale the planar cross-section is a quintic Hermite curve with zero endpoint second derivatives. Its removed area is `setback² * (3/5 - 23*sqrt(2)/70)`. Native tests independently check box and cylinder volumes, concave added wedges, and signed normal/curvature witnesses at the rails. Curved joins cover both cylinder seam frames. This establishes sampled continuity on the checked cases, without certifying every point of every general blend.

See the [qualification record](../blend-edge-qualification.json) for tested shapes, failures, sanitizer configuration and current limits. Licensed Rhino capture remains pending the recorded [Wine startup failure](../oracle-sspi.md). Shape equality, native getter parity and comparative performance remain unqualified.

Variable per-edge/along-edge handles, DistFromEdge, open polysurfaces, TrimAndJoin=No, previous edge sets, chain/face selection, preview, History editing and native saved defaults remain future work. General corner continuity and curved joins beyond the qualified cylinders require broader qualification. Native primitive sources with coarser declared topology zones retain their lower precision; see the qualification record.

References: [Rhino BlendEdge](https://docs.mcneel.com/rhino/8/help/en-us/commands/blendedge.htm), [public BlendType](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_BlendType.htm).
