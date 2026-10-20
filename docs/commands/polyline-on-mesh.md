# PolylineOnMesh

Enter `PolylineOnMesh`, select a polygon mesh, and pick or type locations.
A single preselected mesh is accepted. Enter finishes, `Undo` removes the last
point, and `Close` connects the last point to the first. Returning to the start
after three locations finishes immediately. Escape discards the draft and
releases the mesh selection without consuming Redo.

```text
PolylineOnMesh -3,-2,0 3,2,2.25 Mesh=<object-id>
PolylineOnMesh -3,-2,0 2,-1,0 1,3,0 Close Mesh=<object-id>
```

Pointer input uses the visible mesh sheet along the viewport ray. Typed input
in the getter uses the same view projection; relative coordinates and document
units are supported. Scripted points project along the construction-plane
normal, toward the viewer; the default is Top. A projection miss aborts a
script atomically and is ignored by the point getter.

Between different polygon faces, the path follows a section plane through the
two locations and the starting face normal. Reversing the input can therefore
change the path. Locations within one original face connect directly. This
includes warped quads: the native command does not insert a corner at their
internal triangulation diagonal, so that chord can leave the triangulated mesh.

When a section meets a hole, a bounded graph follows the naked boundary around
it. Disconnected mesh pieces can connect through their original boundary
corners, matching the captured native command: a bridge can leave the mesh or
skip an intervening piece. Equal-length detours use a destination-first search
that preserves the first boundary corner's priority through later ties.
Exact coincident vertices on unwelded faces share section topology without
changing the source mesh.

The implementation reuses mesh projection, scale-safe polygon normals and exact
mesh sections. The getter retains an owned spatial index and accepted sections;
boundary topology is cached lazily. Hovering prepares only the next section.
Enter commits the accepted draft even
while an unpicked segment is under the cursor. Source geometry remains unchanged.
Successful outputs are unselected on the current layer, with no name; source
selection survives. One document Undo removes the entire result, and Redo
restores it. Source, tolerance, selectability or history changes invalidate the
captured draft.

The fixed native cases and their unchanged `1e-6` comparison gate are recorded
in [the qualification report](../mesh-polyline-qualification.json). The captures
use Rhino's Top view and include 17 hole, disconnected-piece, unwelded-seam and
equal-distance cases. Full compatibility remains incomplete: the native
degenerate output after a sequence containing projection misses is not
reproduced. Extra collinear section vertices may remain; native vertex-count
and parameter-clock parity is not claimed. Nonmanifold branches and perspective
numeric-input parity need further qualification. A draft accepts at most 256
locations. Boundary searches allow 16,384 graph nodes, 1,024 components and
4,000,000 work units; ambiguous, unresolved or degenerate paths fail before
document edits. Numerical distance ties use 16 machine epsilons, separate from
the model comparison gate. This routing does not certify a general mesh geodesic.

Reference: [Rhino PolylineOnMesh manual](https://docs.mcneel.com/rhino/8/help/en-us/commands/polyline.htm#PolylineOnMesh).
