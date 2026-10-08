# Boolean2Objects

[Command reference](README.md) · [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/booleanunion.htm#Boolean2Objects)

Select two supported polysurfaces and run `Boolean2Objects`, or start
the command, pick the objects and press Enter. The pending Union result appears
in every viewport. Click in a viewport or type Next to cycle:

```text
Union → Intersection → A−B → B−A → Inverse intersection → Union
```

Enter accepts the shown result; Escape or Cancel restores the original scene
without edits or history. A and B follow source selection order. Source geometry
and history remain untouched while cycling; source picks are released when the
preview starts. Preview scenes and prepared choices are reused on wraparound;
Wireframe, Shaded and Ghosted share the result.
Changed, hidden or locked sources or a changed tolerance cancel the stale preview.

DeleteInput defaults to Yes and can be set during source selection. Yes replaces
the first object in place and removes the second. B−A also retains the first
object's identity and attributes. Inverse intersection replaces that first object
with B−A and adds A−B. No retains both originals and creates new output. All pieces
inherit the first object's layer, attributes, geometry user text and groups.
Results are unselected. One Undo restores the original pair; Redo restores the
accepted result. Option changes persist for the current registry session.
The app also accepts DeleteInput changes while cycling as an extension.

Scripts can select two objects and use Mode=Union, Intersection, DifferenceAB,
DifferenceBA or InverseIntersection to accept directly, or supply Sources=id,id:

```text
Boolean2Objects Mode=Intersection DeleteInput=No
Boolean2Objects Sources=<a-id>,<b-id> Mode=InverseIntersection
```

The current kernel accepts certified closed polyhedral B-reps. All five choices
share one exact original-face arrangement; no rounded result becomes an operand.
Source-face seams remain distinct. Preparation has bounded work and cumulative
output limits. Disjoint and strictly contained pairs fail without document edits;
the native capture establishes those failures before cycling. Empty results and
invalid acceptance requests are rejected.

Eleven owned public recipes ran on private Xvfb under
`VibocerosOracleBooleanTwoVerified20261007` with Rhino 8.32.26160.13001. Seven
succeed, one cancels and three fail before cycling. The
[raw records](../../tools/rhino_oracle/observations/boolean_two_command.json)
retain real bounded mouse sequences, prompts and cycle labels, final boundaries,
counts, mass properties, metadata, IDs, selection and independent Undo/Redo.
Replay checks every completed outcome, boundary witnesses in both directions at
`1e-7`, scalars at `1e-9`, volume/centroid at `1e-10`, identity, metadata and history.
The app replay covers original cycle counts, cancellation, failed preparation,
source purity and history; cache/stale-source tests check reuse and invalidation.
See [provenance](../boolean-two-provenance.json).

A production wgpu/egui inspection on private Xvfb checked viewport source picks,
click cycling, Escape, Enter acceptance, Undo/Redo and display-mode changes during
the pending command. The saved Ghosted view shows A−B with one preview object
while the actual layer pane still counts the two unchanged source objects.
This local inspection does not measure native preview pixel parity.

![Pending A−B in four Ghosted viewports](../images/boolean-two-preview-ghosted.png)

Open/curved sources, arbitrary compound policies, near contacts, restart option
persistence, native preview pixels and relative performance remain unsupported
or unverified. The native driver records final command geometry, not the
proprietary display-conduit geometry at each pending click.

```sh
cargo test --release -p viboceros-command boolean_two
cargo test --release --bin viboceros boolean_two
python3 -m unittest tools.rhino_oracle.test_boolean_two
```
