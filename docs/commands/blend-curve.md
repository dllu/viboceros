# BlendCrv

[Curve commands](curves-index.md) · [Blend API](blend.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/blendcrv.htm)

Enter `BlendCrv`, pick two open curves near the desired ends, then adjust the
preview and press Enter. The source curves stay unchanged unless Trim or Join
is enabled. The result is a minimum-degree NURBS curve with an arc-length domain.

`Continuity_1=G0|G1|G2` and `Continuity_2=G0|G1|G2` independently set position,
tangency or curvature. Each invocation starts at G2/G2. `BlendStart=CurveEnd`
uses the selected natural end; `BlendStart=PickPoint` uses the picked location,
initially pointing toward the source start. Flip reverses that direction.
BlendStart, Trim and Join persist for the application session.

Click a contact to slide it on its source, or a shape handle to change its
length. Click again to accept the preview; held dragging also works. Typed
points project onto the constraint. Shape lengths also accept numbers with
units, fractions and arithmetic. Moving a contact preserves handle lengths.
Esc discards the local adjustment; another Esc cancels the command. `!` aborts
immediately. Previews leave geometry and undo/redo untouched.

Enter `Flip` and pick a source to reverse its blend tangent, or press Enter to
flip both. `Trim=Yes` retains the chosen source portions. `Join=Yes` combines
connected sources and the blend, inheriting the first source's attributes and
groups. Natural ends can join with Trim disabled; interior contacts require Trim.
The entire result commits in one undo entry and is deselected.

Explicit scripting bindings use world points and native contact parameters:

```text
BlendCrv Curve=id1 0,0,0 Curve=id2 10,3,0 Continuity_1=G1 Continuity_2=G2
BlendCrv Curve=id1 0,0,0 Curve=id2 10,3,0 Handle1=0.3cm Trim=Yes Join=Yes
```

`Parameter1/2`, `Handle1/2`, `Bulge1/2` and `Flip1/2=Yes|No` are Viboceros
scripting options. Bulges multiply default handles; an explicit handle replaces
the factor. `Edge=id,index` accepts a read-only edge with Trim and Join disabled,
without native edge qualification. Changed source geometry or tolerance clears
the editor and requests new picks.

[Validation](../blend-curve-validation.json) retains 34 native observations:
25 matched document workflows and nine diagnostics. Both builds check geometry,
attributes, selection, history and 50 3DM round trips, with 759 independent native
point/tangent stations. Rhino reports `Nothing` after these accepted blends;
the record preserves that result alongside the actual document changes. Run:

```sh
python3 -m tools.rhino_oracle.check_blend_curve
```

Captures use private Xvfb; application tests are windowless. G3/G4, closed
sources, SubCrv, point-only endpoints, symmetry/Alt edits, curvature display and
history-based re-editing remain pending. Exact hit-box/depth ranking, native
numeric/Escape trajectories, arbitrary self-blends, continuous distance and
comparative speed remain unqualified.
