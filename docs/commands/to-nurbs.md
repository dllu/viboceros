# ToNURBS

```text
ToNURBS DeleteInputObjects=No
ToNURBS DeleteInputObjects=Yes
ToNURBS MeshOptions TrimTriangularFaces=Yes DeleteInputObjects=No
ToNURBS TrimTriangularFaces=No
```

Converts selected lines, circular arcs/circles, polylines, and polycurves to exact
NURBS curves, and triangle/quad meshes to NURBS B-reps. Points and point clouds
are ignored. Existing NURBS curves, surfaces, and B-reps are unchanged. Ellipses
are also no-ops: Rhino already stores them as NURBS curves; Viboceros retains
its equivalent compact analytic representation. Headless execution with no eligible
selection is an error; the GUI instead starts an object-selection prompt.

With preselection, `DeleteInputObjects=No` creates unselected copies with the source's attributes,
layer, name, color policy, and ordered group memberships. Sources and selection
remain unchanged. `Yes` replaces geometry while preserving source identity,
attributes, memberships, and selection. Replaced objects move after unchanged
objects in chronological document order; their relative source order is retained.
Command-first picks instead determine output/replacement order, and all prompted
picks are unselected on completion, including no-op inputs. Both paths retain
the same attribute, identity, and group policies.
This is not the fresh-attribute policy of [Bézier](beziers.md) or
[single-span conversion](single-spans.md).

Curve intervals are retained. Polylines preserve every existing vertex parameter,
not a newly assigned chord-length map. Conics retain their exact locus but acquire
rational parameterization; equal numeric parameters need not identify equal
interior points. See [curve parameter correspondence](../curve-parameters.md).

Each mesh becomes one B-rep, including disconnected components. `TrimTriangularFaces=Yes`
uses rectangular bilinear surfaces trimmed along a diagonal. `No` uses untrimmed
bilinear surfaces with one singular side. Quads become bilinear surfaces; shared
mesh topology becomes shared B-rep vertices/edges. This command does not split
disconnected outputs as `MeshToNURB` does. Mesh options require a selected mesh.
The native command accepts the optional `MeshOptions` marker or the flat option;
Rhino scripts enter the `MeshOptions` subprompt before setting triangle trimming.

Deletion and triangle-trimming choices are remembered independently of the span
commands and survive undo. Unlike `ConvertToSingleSpans`, a `ToNURBS` no-op does
**not** accept new choices. Native bootstrap choices are No/Yes respectively;
application-restart persistence and Rhino factory defaults are not established.
See [option memory](../command-options.md).

## Interactive workflow

With no eligible preselection, enter `ToNURBS`, click or window-select objects,
then press Enter to open conversion options. Picks are additive and do not expand
groups; Ctrl/Command removes picks. Points and point clouds are excluded.
With eligible preselection, the GUI goes directly to confirmation. No-op-only
selections finish without offering options or remembering new choices.

At confirmation, set `DeleteInputObjects=Yes|No`, or enter `MeshOptions` when a
mesh is selected. Inside that submenu, set `TrimTriangularFaces=Yes|No` and press
Enter to return to the main options. Enter at the main options performs the
conversion. Unlike [MeshToNURB](mesh-to-nurb.md), finishing object selection does
not convert immediately. Selection is fixed during confirmation; viewport
navigation, display controls, and nested CPlane input remain available.

Escape cancels the entire command, including from MeshOptions. No options are
remembered, no geometry is edited, and undo/redo history is unchanged. Existing
preselection is retained; command-first picks and initial ineligible selection
are cleared. See [object prompts](../object-selection.md) for architecture and input rules.
The headless command API executes its complete typed invocation directly.

## Implementation and verification

The independent `to_nurbs` command stages all geometry before one undoable edit.
Mesh conversion reuses the exact `Brep::try_from_mesh` topology assembler. Aggregate
output is bounded to 1,048,576 NURBS controls (four per mesh face). Resource or
geometry failures leave sources, attributes, groups, selection, history, and
remembered choices unchanged. This budget is not an approximation tolerance.

The document `object_order` module renews chronological order independently of
geometry. A linear-time permutation and its inverse preserve object contents;
history stores only moved IDs and original indices, not geometry snapshots for
ordering. Explicit ordered renewal supports pick order as well as stable document
order. Forward and inverse cycle replay use one index vector in linear time;
inverse replay does not allocate another permutation. Tests exhaust all 256
subsets of eight objects and all 326 ordered subsets of five objects, and compose ordering with
insertion, deletion, geometry replacement, rollback, and repeated undo/redo.
Ordered geometry copies share the existing attribute/membership and atomic
preflight implementation, but insert outputs in caller order.
Other commands' replacement-order policies are not established by this audit.

The 78 `nurbs_conversion.json` comparisons run the actual Rhino command. They
check full curve/surface NURBS definitions, representation kind, native domains,
sampled geometry, B-rep topology and trim endpoints, source identity, creation
order, attributes, ordered memberships, selection, and complete group tables.
Six `nurbs_conversion_sessions.json` sequences add 39 steps checking option
independence, partial changes, no-op rejection of choices, and undo. Every session
must seed choices with a real conversion, and seed triangle trimming at the first
mesh; a no-op cannot establish known Rhino state. Comparisons use absolute `1e-8`,
relative `1e-12`; maximum observed error is `2.67e-15`.

The 87 `nurbs_postselection.json` cases add prompted selection across the same
geometry families, partial/reversed picks, mixed no-op inputs, initial point
selection, and preselected cancellation. Three `nurbs_postselection_sessions.json`
sequences add 31 steps checking cancellation before/after selection, cancelled
preselection, no-op rejection of options, undo, partial updates, and independent
MeshToNURB memory. All 90 comparisons pass at the same epsilon and maximum error.
Separate private-Xvfb Rhino mouse/keyboard probes confirm that clicking one grouped
mesh does not select its other members, and Escape inside MeshOptions cancels
without geometry output.

The shared conversion codec pads only the two unused outer knot slots omitted by
OpenNURBS; every stored knot is still compared. No attributes or ordering fields
are normalized to hide differences. A separate Rhino object-enumeration probe
confirmed chronological renewal on replacement.

A private-Xvfb GUI check imports a custom-parameter polyline, disconnected mesh,
and point on a non-current layer. Six exported 3DM models verify exact geometry,
triangle trims, chronological order, source attributes, populated/empty groups,
both deletion choices, remembered options, no-op behavior, and undo/redo. The
final model was also inspected in all four ghosted viewports.
An additional nine-export GUI check exercises the two-stage workflow and submenu,
frozen confirmation picks, both deletion choices, pre/postselection ordering,
cancellation, remembered choices, no-ops, and undo/redo. Exact source and output
geometry, native polyline parameters, triangle trims, attributes, and group tables
match the expected models. Display and CPlane changes preserve confirmation state.

SubD conversion, mesh/SubD edge subobject conversion, and lightweight extrusion
objects are not implemented. Meshes currently contain triangle/quad faces, not
Rhino n-gon regions. General performance parity is not established. These are
explicit gaps relative to the [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/tonurbs.htm),
not alternative definitions of the command. Runtime attribute/order policies
above were measured with the licensed oracle; its curve/mesh copies retain
source layers despite the reference's general current-layer description.
