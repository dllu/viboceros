# ToNURBS face-pack controls

```text
ToNURBS SubDOptions Faces=Packed ShowFacePacks=Yes ForceUpdateFacePacks=Yes
```

After selecting SubDs, open `SubDOptions`. `ShowFacePacks` and
`ForceUpdateFacePacks` toggle immediately when entered alone; both also accept
`=Yes` or `=No`. They reset to No on each invocation. Faces and extraordinary
vertex processing retain their ordinary conversion preferences.

`ShowFacePacks` colors the current conversion groups temporarily. Forced packing
updates the preview; switching it off restores retained valid groups. Missing or
stale packs use the same topology rules as conversion. The preview uses cached
source snapshots and the display mesh's face map, supports flat and smooth SubD
display, and appears in wireframe, shaded and ghosted views. It does not assign
object or per-face colors. Its stable palette is independent of Rhino's palette.

`ForceUpdateFacePacks` rebuilds packing on private candidates. Accepting updates
retained source packing and renews source creation order before adding output
objects. Source updates and conversion form one undoable edit. Cancellation and
conversion failures preserve original source geometry, packing and redo.
Deleting inputs retains the ordinary ToNURBS replacement behavior. The independent
texture atlas and algorithm UUID follow [PackSubDFaces](pack-subd-faces.md).

Choosing `Faces=Unpacked` hides both controls and the preview while retaining
their values. A previously enabled force choice still updates source packing on
acceptance. Returning to Packed restores the controls and their values.

Two immutable native captures cover 14 workflows and 10 complete B-reps. A native
quad sphere starts with two custom packs; forced conversion produces six.
Qualification covers toggle reversal, per-invocation resets, hidden controls,
both deletion choices, reversed prompted picks, source order, cancellation and
undo/redo. Complete surface images and oriented B-rep topology match at `1e-11`;
observed surface and edge bounds are at most `1.1e-13` and `1.2e-13`.
UI and scene tests check cancellation, redo, stale snapshots, color caching and
source-face mapping without opening windows. Native captures use private Xvfb.

See [whole-SubD conversion](to-nurbs-subd.md) for approximation limits and the
[Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/tonurbs.htm).
