# MeshFromLines

`MeshFromLines` builds a polygon mesh from line segments connected at their
endpoints. Select the lines first, or start the command and pick them. Enter
builds the mesh; Escape cancels without consuming Redo.

```text
MeshFromLines
MeshFromLines MaxSidesPerFace=6 DeleteInput=No
MeshFromLines Sources=<id>,<id>,<id> OutputLayer=Current
MeshFromLines Sources=<id>,<id>,<id> OutputLayer=<layer-id>
```

`MaxSidesPerFace` defaults to 4 and accepts integers of at least 3. It limits
the size of recovered boundary loops. A lone four-line rectangle produces no
mesh when the limit is 3; a pentagonal loop needs a limit of at least 5. Loops
larger than four sides reuse constrained polygon triangulation.

`DeleteInput` defaults to No. When enabled, successful construction removes
the participating lines in the same Undo operation as the new mesh. Inputs
remain when no closed region is recovered. Undo restores deleted sources
unselected; preselected objects rejected by the line filter keep their selection.
Polyline objects are excluded. Exact linear NURBS curves are eligible.

`OutputLayer` defaults to Current. The application also accepts a layer ID;
the interactive OutputLayer prompt accepts existing layer names. Targeting
another layer leaves the current layer unchanged. The new mesh is unnamed
and unselected. Face-size and deletion preferences belong to the command
registry and survive model Undo; independent registries start with defaults.

Endpoints within the model tolerance connect without modifying the source
curves. Interior crossings do not introduce mesh vertices: split lines at
intended junctions before meshing. The recovered regions can depend on line
order. The captured cube network produces five quads, so a closed edge network
does not guarantee a closed shell. Command output separates creases at the
captured 60-degree boundary using float face-normal comparisons, while retaining
double-precision vertex positions.

Construction is bounded to 100,000 input lines and 4,000,000 work units. Dense
networks fail before document edits. Region enumeration uses an explicit stack;
larger polygon triangulation also follows the shared patch limits.

The [qualification report](../mesh-from-lines-qualification.json) retains 60
public SDK captures and command/getter replays. Geometry checks retain the
`1e-6` gate and compare vertex and face counts. Source storage, attributes,
selection, deletion and grouped Undo/Redo are also checked.

Five captured inline option spellings are rejected by the native build,
including `CurrentLayer` and attempted non-current layer values. Their token
rows remain unqualified; replays compare the effective accepted Current-layer
settings. Explicit layer-ID targeting is a local extension. General nonmanifold
or nonorientable networks, ambiguous large polygons, native face ordering and
comparative speed need further qualification.

References: [McNeel MeshFromLines command](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshfromlines.htm)
and [public Mesh.CreateFromLines API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.mesh/createfromlines).
