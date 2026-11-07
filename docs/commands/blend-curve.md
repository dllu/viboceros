# BlendCrv

[Curve commands](curves-index.md) · [Blend API](blend.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/blendcrv.htm)

Enter `BlendCrv`, pick two open curves near the desired ends, then adjust the
preview and press Enter. The source curves stay unchanged unless Trim or Join
is enabled. The result is a minimum-degree NURBS curve with a domain scaled to its measured length.

`Continuity_1=G0|G1|G2|G3|G4` and `Continuity_2=G0|G1|G2|G3|G4` independently
set endpoint constraints. G0/G1/G2 use position, tangency and curvature; G3/G4
follow the measured Native default construction described below. Each invocation
starts at G2/G2. `BlendStart=CurveEnd`
uses the selected natural end; `BlendStart=PickPoint` uses the picked location,
initially pointing toward the source start. Flip reverses that direction.
BlendStart, Trim and Join persist for the application session.

For one endpoint, enter `Point` and pick or type a location instead of selecting
a curve. The point is locked to G0; the curve can use G0–G4. At least one endpoint
must be a curve. Point placement and movement use ordinary drafting, object
snaps and 3D typed coordinates. No point object is created or consumed. While
picking the location, `Point` requests a one-shot point snap.

Enter `SubCrv` while selecting a source to use a temporary portion. Select its
parent (including a closed curve), then its start and end. Point order sets the
portion direction; the last endpoint becomes the blend contact. A numeric length
accepts units and uses the cursor side; `Direction=Locked` holds that direction
and `Direction=Free` releases it. Numeric intervals can wrap a closed seam.
PickPoint initially points back toward the portion start. Esc discards the
nested getter and returns to source selection.

Click a contact to slide it on its source, or a shape handle to change its
length. Click again to accept the preview; held dragging also works. Typed
points project onto the constraint. Shape lengths also accept numbers with
units, fractions and arithmetic. Moving a contact preserves handle lengths.
Esc discards the local adjustment; another Esc cancels the command. `!` aborts
immediately. Previews leave geometry and undo/redo untouched.

Hold Shift while selecting a shape handle to set both handle lengths together.
Both ends must have the same G1–G4 continuity. Each handle stays on its own tangent
constraint; contacts and mixed/G0 constraints edit independently. Pairing lasts
for that adjustment, including when Shift is released before mouse-up.

Enter `Flip` and pick a source to reverse its blend tangent, or press Enter to
flip both. `Trim=Yes` retains the chosen source portions. `Join=Yes` combines
the blend with source portions when Trim is enabled. With Trim disabled, only
inputs meeting the blend at an endpoint participate; disconnected
sources remain unchanged and outside the joined result. The output inherits the
first participating whole source's identity, attributes and groups. Temporary
portions can join without consuming or trimming their parent objects; a whole
input sharing that parent still participates normally. If none participates, it is
a new PolyCurve containing the blend and any participating temporary portions,
on the current layer. The entire
result commits in one undo entry and is deselected.

Explicit scripting bindings use world points and native contact parameters:

```text
BlendCrv Curve=id1 0,0,0 Curve=id2 10,3,0 Continuity_1=G1 Continuity_2=G2
BlendCrv Curve=id1 0,0,0 Curve=id2 10,3,0 Handle1=0.3cm Trim=Yes Join=Yes
BlendCrv Point=0,0,0 Curve=id2 10,3,0 Continuity_2=G2
BlendCrv SubCrv=id1,1,4 Curve=id2 10,3,0 Trim=Yes Join=Yes
```

`SubCrv[Wrap]=id,start,end` uses native parent parameters; `Wrap` selects a
directed interval that may cross a closed seam. `SubCrv=id,Edge,index,start,end`
also accepts private edge portions, without native edge qualification.

`Parameter1/2`, `Handle1/2`, `Bulge1/2` and `Flip1/2=Yes|No` are Viboceros
scripting options. Bulges multiply default handles; an explicit handle replaces
the factor. `Edge=id,index` accepts a read-only edge with Trim and Join disabled,
without native edge qualification. Point endpoints have no curve parameter,
shape handle or flip direction. Changed source geometry or tolerance clears
the editor and requests new picks.

Validation: [G0–G2](../blend-curve-validation.json),
[G3/G4](../blend-curve-high.md), [point endpoints](../blend-curve-points.md),
[temporary sources](../blend-curve-portions.md), and [Shift editing](../blend-curve-symmetry.md).
Both builds check geometry, attributes, identities, selection, history, 3DM round
trips and independent Native evaluations. Rhino reports `Nothing` with
success=false after accepting these blends; records retain the actual document
changes and separate geometry, derivative and domain-precision gates.

G3 uses the normal third source jet. Native G4 defaults use a **zero fourth
blend derivative**, including on sources with nonzero fourth derivatives.
This reproduces the measured command behavior; it does not establish mathematical
G4 continuity for arbitrary sources. Default degrees are seven for G3/G3 and
nine for G4/G4; mixed endpoints use degree `order1 + order2 + 1`.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_symmetry
```

Captures use private Xvfb; application tests are windowless. Whole closed sources,
zero-retained-side joins, surface-edge Alt edits, curvature display and
history-based re-editing remain pending. Exact hit-box/depth ranking, native
numeric/Escape trajectories, arbitrary self-blends, continuous distance and
comparative speed remain unqualified.
