# Architecture

[Project overview](../README.md) · [Documentation](README.md) · [Development and testing](development.md)

Dependencies run from geometry through document state and commands to the
application. Geometry has no dependency on egui, command parsing or file formats.

## Crate boundaries

| Crate | Owns |
| --- | --- |
| `viboceros-geometry` | Validated primitives, NURBS, B-rep topology, meshes, SubDs and geometry queries |
| `viboceros-opennurbs` | Shared OpenNURBS build and native geometry/archive adapters |
| `viboceros-smlib` | Optional usd-brep/SMLib bridge and owned native modeling operands |
| `viboceros-document` | Objects, immutable geometry snapshots, attributes, layers, groups, blocks and history |
| `viboceros-io` | 3DM, STL and STEP interchange and unit conversion |
| `viboceros-drafting` | Typed coordinates, construction-plane constraints, object snaps and tracking |
| `viboceros-command` | Parsing, per-command settings, selection policy, staged outputs and atomic admission |
| `viboceros-oracle` | Independent native comparisons and headless command execution |
| Application | egui/wgpu UI, viewports, transient tools, previews and document presentation |

The [native modeling bridge](native-modeling.md) delegates supported B-rep and
NURBS operations to usd-brep. Rust retains document and command policy; copied
operands and serialized native calls keep ownership explicit. The OpenNURBS
adapter supplies public SubD construction, evaluation and archives, and shares
one native build with file interchange. Native [extrusions](extrusions.md) retain
an immutable archive alongside their validated canonical B-rep view. Portable and native capabilities differ;
see the [file-format guide](file-formats.md) and individual command limits.

## Geometry and command integration

Geometry operations validate inputs and produce candidates before changing the
document. Shared geometry modules own algorithms and numeric conventions;
commands own selection, attributes, resource limits and acceptance policy.
Immutable source snapshots and tolerance checks detect stale prepared inputs.

Commands stage a complete edit before atomic admission. Interactive tools keep
unfinished points, reference contacts and previews outside the document, then
commit one constructive history group. Undo/Redo restores document snapshots;
camera, prompt and hover state remain transient. Primitive commands reuse
rectangle, circle, ellipse and axis getters with independent remembered settings.

## Viewport and drafting boundaries

Camera projection, extents, clipping, picking, display and drafting live in
separate viewport modules. Drafting resolves typed or picked input against a
viewport's construction plane without changing its camera. Object and component
selection share geometry snapshots and visibility rules; temporary component
choices stay outside model history. Display meshes and prepared queries are
derived caches, invalidated when their source geometry or tolerance changes.

See [viewport boundaries](architecture/viewport.md) for the module map and
[viewport precision](viewport-implementation.md) for coordinate conventions.

## Current foundation

The model supports analytic and rational NURBS geometry, trimmed B-reps, mixed
polygon meshes, SubDs, point clouds and block instances, with layers, groups and
bounded history. Rhino compatibility remains incomplete; the
[command reference](commands/README.md) records supported workflows, and each
qualification separates measured comparisons from remaining limits. Rhino Render
is outside scope.

## Implementation notes

- [Native kernels and numerical geometry](architecture/kernel.md)
- [Document objects and editing](architecture/document-editing.md)
- [Command construction and staged input](architecture/command-input.md)
- [Geometry integration and representation](architecture/geometry.md)
- [Analysis, curve queries and qualification](architecture/qualification.md)

Keep this overview about ownership, dependencies and shared contracts. Put
feature syntax in command pages and detailed algorithms or evidence in the
relevant subsystem guide.
