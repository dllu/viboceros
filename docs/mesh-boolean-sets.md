# Multiple mesh targets and MeshBooleanSplit

`MeshBooleanDifference` preserves separate output slots for its input targets,
including untouched peers when another target is cut. Results inherit the first
target's name, layer, color, and groups. That target retains its identity; other
results are new objects. Preselection follows document order, while later picks
follow selection order. A severed target still produces one mesh containing its
separated pieces.

`MeshBooleanSplit` uses the same two-set getter and tolerance modes. It retains
its cutters. A productive cutter returns outside and inside results for meshes
it cuts; untouched working meshes carry through. Disconnected pieces within a
result remain one mesh. Following
the captured Rhino behavior, later cutters process the entire retained result
list and append their results. Earlier stages remain present, so results from
multiple cutters can overlap. No-op cutters add no stage. `DeleteInput` is not
a Split option. Accepted tolerance choices survive cancellation and are shared
with the other mesh Boolean commands.

The [qualification](mesh-boolean-sets-qualification.json) compares 64 command
invocations and 67 application workflows with saved native scenes. Fifty command
invocations and 53 getter workflows match, including geometry, source identity,
output order, selection, memberships, attributes, Undo, and Redo. The two rejected
`DeleteInput` probes remain separately recorded.

Fourteen cases remain explicit gaps: native facet weighting for overlapping or
nested targets; coincident and face-contact results; and empty outside meshes
when a target or working region lies inside its cutter. The geometry model
currently requires nonempty meshes. A fully enclosed target reports no split;
a cutter that only encloses existing regions adds no stage.

Two independent two-cutter captures each contain one completed Split command and
five result meshes with total oriented volume 48 for a target of volume 24. A
three-cutter capture produces twelve results totaling 96. The implementation
replays this retained-history behavior. Analytic Rust tests check closed topology,
unchanged shared cutters, and stage volumes of 24/24 and 24/24/48 respectively.

Curved surface meshing, output vertex channels, extremes, custom tolerance
coefficients, and comparative performance remain unqualified. The
[baseline evidence](mesh-boolean-commands.md) describes the shared comparison
method and the original three commands' separate gaps.

```sh
cargo test -p viboceros-command --features native-smlib --test mesh_boolean_command_native -- --nocapture --test-threads=1 > /tmp/mesh-boolean-commands.log 2>&1
cargo test -p viboceros --features native-smlib mesh_boolean -- --nocapture --test-threads=1 > /tmp/mesh-boolean-getters.log 2>&1
python3 -m tools.rhino_oracle.report_mesh_boolean_sets /tmp/mesh-boolean-commands.log /tmp/mesh-boolean-getters.log
```
