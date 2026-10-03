# Mirror planes

[Transforms](transforms.md) · [Copy and history](transform-copy.md)

With objects selected, start Mirror and pick two points for a plane perpendicular
to the current CPlane. At the first point prompt, choose one of these options:

| Option | Mirror plane |
| --- | --- |
| 3Point | Pick an origin, a second point, and a noncollinear third point to define a spatial plane. |
| XAxis | The active CPlane's XZ plane, through its origin. |
| YAxis | The active CPlane's YZ plane, through its origin. |
| ZAxis | The active CPlane itself, through its origin. |
| Object | Pick a planar surface or polysurface face as a spatial mirror plane. |

Axis shortcuts execute immediately. These choices follow
[Rhino's documented Mirror options](https://docs.mcneel.com/rhino/8/help/en-us/commands/mirror.htm).
They can also be supplied when starting the command, for example
`Mirror 3Point Copy=No`, then pick the plane points. Edit Copy at any of its point
prompts. Set Copy before entering the Object target prompt; native Rhino rejects
Copy edits at that prompt. Plane choices are available before the first two-point
start is picked; later choices are rejected while the point prompt remains active.

Full registry invocations use world point arguments:

```text
Mirror 3Point 1,2,3 3,4,5 4,0,7 Copy=Yes
Mirror XAxis Copy=No
Mirror Object <object-uuid> Face=2 Copy=Yes
```

A repeated second point is rejected. A duplicate or collinear third point ends
the command without an edit, matching native plane-construction failure. Enter or Escape
at earlier prompts also leaves geometry unchanged. A completed Mirror finishes
after one edit. Point-defined mirrors retain selected originals and unselected
copies. Choosing Object clears source selection and retains the source IDs in
the pending transform; completion, rejection, and cancellation leave those
sources unselected. Picking the plane target never adds it to the source set.
The target stays unchanged. Enter an object UUID at this prompt, optionally
followed by `Face=index`, or click a surface or face in a viewport. Polysurfaces
require a face. Curved surfaces, curves, meshes, missing faces, and unselectable
objects are rejected while the target prompt stays active. A surface converted
from an extrusion is represented as a B-rep and supports the same face picking.
[Attributes, group definitions, object order, SelLast, and Undo/Redo](transform-copy.md)
follow the shared affine-command policy.

## Verification and limits

The [64 prescribed cases](../../tools/rhino_oracle/fixtures/mirror_planes.json)
compare three-point planes and all three axis shortcuts on WorldXY, translated
tilted, and translated Right CPlanes. Four selected, affinely independent point
witnesses distinguish the entire spatial affine map; an unselected fifth point
checks replacement order. Grouped copies, a single selected group member, Copy
edits, cancellation, degenerate points, and late option attempts are included.
The [raw Rhino 8.32 observations](../../tools/rhino_oracle/observations/mirror_planes.json)
retain histories, terminal Success/Cancel/Failure events, geometry, attributes,
selection, source identity, group topology, SelLast, and external Undo/Redo.
App prompts and complete registry invocations match document snapshots at
absolute epsilon `1e-9`, relative zero; automatic group names are raw telemetry.
The [five early-Enter recipes](../../tools/rhino_oracle/fixtures/mirror_enter.json)
and their [raw capture](../../tools/rhino_oracle/observations/mirror_enter.json)
verify command termination at each unfinished two-point or three-point phase.
These macros end at Enter, with no trailing Cancel token.

The [70 Object recipes](../../tools/rhino_oracle/fixtures/mirror_object.json)
and [raw native capture](../../tools/rhino_oracle/observations/mirror_object.json)
cover horizontal, vertical, and tilted surface targets; planar trimmed faces;
all six box faces; and all four extrusion wall borders. Face inputs are actual
owned mouse clicks. Extrusion border recipes explicitly choose the first entry
from the owned selection menu; no result geometry supplies input coordinates.
Grouped and ungrouped copies, a single group member, target rejection, early
termination, late Object choices, and rejected Copy edits are included.
The replay exercises separate prompts, options on the starting line, and complete
registry invocations. Two recipes omit SelLast before Undo: native Redo restores
the prior output selection, including unselected outputs. Target control points,
topology, attributes, and selection are recorded before and after; native cached
bounding boxes are raw telemetry and can shift after float-precision meshing.

These witnesses verify affine maps and document behavior, without exhaustively
comparing every curve, surface, or mesh representation. History-linked planar
targets, subcurve input, and command-first source selection remain incomplete.
Near-collinear acceptance thresholds are not measured by these captures.

## Live viewport preview

After the first axis point, moving the cursor previews the reflected objects in
all viewports. `3Point` starts its preview after the second plane point. The preview
uses the resolved drafting point, including object snaps, coordinate filters, and
point constraints, and the picking viewport's construction plane.

Reflected wires and points use the selection color. Shaded and ghosted reflected
faces retain their object or layer color. Original objects remain visible as wires:
`Copy=Yes` retains their display color for the two-point plane, while `Copy=No` and
`3Point` use gray reference wires. Changing Copy updates the display immediately.
Coincident or collinear final points retain the last valid preview; accepting a
point still runs the command's independent validity checks. Leaving the viewports
retains the preview, and completion or cancellation removes it.

Previews transform cached wire samples, display meshes, and normals while staging
the GPU scene. Mouse movement does not clone model geometry, retessellate surfaces,
edit selection, change persistent command defaults, or create history entries.
Clipping includes reflected bounds so previews can extend beyond the source scene.

The [15 native preview cases](../../tools/rhino_oracle/fixtures/mirror_preview.json)
cover two-point and three-point planes, Copy settings, all three display modes,
cancellation, and movement from a valid plane onto a degenerate final point.
[Raw observations](../../tools/rhino_oracle/observations/mirror_preview.json) retain
pending document snapshots, final bounds, calibrated cursor locations, and checksums
of private Xvfb framebuffer captures. These are behavioral witnesses, not a claim
of pixel-identical Rhino rendering or exhaustive geometry coverage.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/mirror_planes.json --scheme VibocerosOracleMirrorPlanesFresh --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/mirror_enter.json --scheme VibocerosOracleMirrorEnterFresh --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/mirror_object.json --scheme VibocerosOracleMirrorObjectFresh --timeout 300
cargo test -p viboceros mirror_plane_options --bin viboceros
cargo test -p viboceros mirror_object_ --bin viboceros
python3 -m unittest tools.rhino_oracle.test_transform_copy
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.mirror_preview_input tools/rhino_oracle/fixtures/mirror_preview.json --output /tmp/mirror_preview.json --scheme VibocerosOracleMirrorPreview --timeout 240
cargo test -p viboceros --bin viboceros mirror_preview
cargo test -p viboceros --bin viboceros gpu_mirror_preview -- --ignored --nocapture
python3 -m unittest tools.rhino_oracle.test_mirror_preview
```

Use a private oracle settings scheme and the dedicated Xvfb wrapper for capture.
Preview capture additionally requires Pillow and xdotool on the host.
