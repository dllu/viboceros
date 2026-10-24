# MeshTrim native getter checks

[MeshTrim](commands/mesh-trim.md) now has productive native preselection captures
for a sheet and a closed box. The oracle starts the command's own region getter,
delivers a physical click, then sends Enter. This direct getter path produces
productive preselection where the earlier pause macros did not. This changes the oracle input
driver; the modeling kernel's cutter projection remains unchanged.

Optional managed mouse callbacks record the actual viewport, pixel, getter
prompt and preceding pointer motion. Separate public pick-frustum queries record
native object hits and their 3D points. Callbacks remain entirely managed while
Python awaits the command, and are disabled on both success and failure. These
queries are diagnostic controls and are not assumed to be interchangeable with
an unobserved command run.

The [qualification](mesh-trim-getter-qualification.json) retains 29 captures,
including 20 native-only diagnostics. Nine backend and nine getter workflows
are replayed; six in each set match surfaces, attributes, channels, selection,
source storage, Undo and Redo. Four workflows exercise productive preselection,
including two using the input driver's default path.

Three independent SDK controls split private copies of the vertical target.
Their inputs remain unchanged, and their parts cover the original area of 8:

| Cutters | SDK part areas | MeshTrim retained area |
| --- | --- | --- |
| Vertical plane mesh | 4, 4 | 4 |
| Horizontal plane mesh | 4, 4 | 4 |
| Both plane meshes | 2, 2, 2, 2 | 8 |

The two-cutter command result differs from removing the picked SDK region,
which would retain area 6. This also reproduces with mesh cutters and without
scripted pauses, so a curve-only projection change would not explain the
observations. The underlying native behavior remains unresolved. Both direct
temporary-plane controls still leave Rhino's target unchanged; Viboceros trims
it. These remain compatibility gaps, with the original observations preserved.

Curve extrusion along the current CPlane Z axis follows the documented
[MeshSplit contract](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshsplit.htm).
The [earlier continuation checks](mesh-trim-continuations.md) retain their
separate measurements and perspective curve cases.
