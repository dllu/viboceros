# Headless Python modeling

Build Viboceros with `cargo build --release` after following the dependencies
and submodule setup in the [README](../README.md#build-and-run).
Run your script from the checkout with Python 3.10 or newer. To import it
from elsewhere, install the Python package with `python -m pip install -e .`
and put the built `viboceros` executable on `PATH` or set `VIBOCEROS_BIN` to its
path. Use `Session(executable=...)` to select a specific executable.

```python
from viboceros import ENTER, Pick, Session

with Session() as model:
    body, = model.command("Box", (0, 0, 0), (38, 25, 0), 12).added
    tool, = model.command("Box", (25, 8, -2), (42, 17, -2), 16).added
    cut = model.run("BooleanDifference", Pick(body), ENTER, Pick(tool), ENTER)
    part, = cut.added
    model.command("SetObjectName", "Housing", select=[part])
    model.select()
    model.save("housing.3dm")
    model.export("housing.step")
    for view in ("Perspective", "Top", "Right"):
        model.screenshot(f"housing-{view.lower()}.png", view=view)
```

A session owns a document and all of its command, selection, viewport, and undo
state. Use `run()` with any supported command line, one line per argument. The
[command reference](commands/README.md) describes each command's arguments.
`command()` formats regular arguments and `options={"DeleteInput": False}`
as command input. Use `run()` to give exact command syntax or answer getters.
`commands()` reports the application's completion catalog. Sessions can also
run the application's `ReadCommandFile` command.

## Getters and selection

- Pass whole command strings or separate prompt answers to `run()`. A tuple
  `(x, y, z)` or `Point(x, y, z)` is a typed world point. Use
  `Point(x, y, z, world=False)` for a construction-plane point. Pass `ENTER`
  to accept a stage and `ESCAPE` to send Escape. `result.pending` and
  `result.prompt` report the command's current input state.
- `objects()` returns object IDs, kinds, names, layer and group IDs, and display
  bounds. Filter by `kind`, `name`, or `layer`. Use `select(*objects)` or
  `run(..., select=[...])` for preselection in the supplied order. Supply IDs
  or the object records themselves. `Pick(obj)` makes an in-command pick.
- `components(obj)` lists zero-based B-rep, mesh, and SubD topology references.
  `select_components(*refs)` preselects them and `Pick(ref, at=(x, y, z))`
  submits one to the active getter with an optional location. Refresh component
  references after geometry changes. `grips(obj)` lists the enabled
  control or edit points. Use `select_grips(*grips)` or `Pick(grip)`.
- `PickPoint((x, y, z), view="Top")` projects a point and uses the viewport's
  usual hit testing. `ActivateView`, `Key`, and `Pointer` cover view activation,
  modifier keys, and screen-space clicks or drags. `viewports()` gives each
  viewport's rectangle and `project()` maps a world point to a screen position.
- Use `widgets()` to inspect dialog controls. `WidgetAction("Label")` clicks a
  uniquely labeled control. Pass a widget record to target it by ID, or use
  `WidgetAction(widget, "focus")` followed by `TypeText(text)` to enter values.
- Results carry the console output in `messages` and the added, deleted, and
  geometry-modified object IDs. Pass `track_changes=False` to omit the object
  comparison for large batches. A reported error raises `CommandError`, with
  the partial result on the exception's `result` attribute. Pass `check=False`
  to keep submitting steps after errors. Earlier edits remain available to Undo.

## Exact surfaces

`NurbsSurface` takes degrees, full knot vectors, and a control net indexed
as `control_net[u][v]`. Supply optional weights with the same layout. Add a
surface with `add_nurbs_surface()` or use `add_surface_patches()` to extract
several exact rectangular UV subdomains in a single undo step. Each patch
keeps the parent's parameterization and can have its own name, color, and
opacity. `opacity=0.25` makes a surface translucent in shaded views. Opacity
ranges from 0 for clear to 1 for opaque and is stored in the object's 3DM
material. Read it from `model.object(id).opacity`.

Use `trim_loops` on a patch or in `add_nurbs_surface()` to create a B-rep face
on that support. Supply a simple, counterclockwise outer loop of connected
`TrimCurve` segments in native UV, followed by any disjoint clockwise holes.
Each segment uses a full knot vector. `TrimCurve.line(start, end)` is a
convenience for straight segments in UV.

Read controls and knots from an object with `surface_definition()`. Use
`surface_jets(obj, (u, v), ...)` to inspect the point and all partials through
order two in native UV. `SurfaceSample` specifies left or right limits at
knots. Pass `face=` to query the supporting surface of a B-rep face.
`pullback_brep_edge(edge, obj, face=...)` returns a `TrimCurve` whose
entire spatial image matches the named edge at document tolerance.
Use `reversed=True` to reverse the result or supply `endpoints=(start, end)`
to match the UV endpoints of neighboring trims.

Use `sew_brep_edges((first, second, reversed), ...)` when the mating edges
are known. Get each edge from `components()`. Set `reversed` to `True` for
oppositely directed edges. Sewing preserves each face's UV support and
replaces the input B-reps in one undo step. Use `anchor=object_id` to retain
the orientation and attributes of a specific input body.
For a replacement face on an existing seam, `vertex_distance=` can bound
displacement of previously shared vertices independently from the curve match.
To split several edges of one B-rep in a single operation, use
`model.run(f"SplitEdge {object_id} Batch 0:2,6 1:3")` with each edge's
original index and its curve parameters.
Use `ShrinkTrimmedSrfToEdge object-id PoleEdges=0,1` to represent named
roundoff-sized naked edges as singular trims. This requires the corresponding
surface sides to collapse to the same points.

## Files and images

Use `save()` for the current project or `save(path)` for Save As. `open(path)`
replaces the document with a 3DM project. `import_file(path)` and `export(path)`
choose 3DM, STL, or STEP by extension. Pass `native_step=True` to use the native
STEP option on a compatible build, or `ascii_stl=True` to write text STL.
For 3DM, pass `selected_only=True` to export the current object selection.

`model.screenshot()` writes PNG using the desktop viewport's renderer. Give it a
viewport name or index, a saved named view, or a standard direction including
`Perspective`, `Top`, `Bottom`, `Front`, `Back`, `Right`, and `Left`. It uses
shaded mode and fits visible geometry by default. Set `mode="Wireframe"` or
`mode="Ghosted"`, `fit=False` to keep the camera, or supply `size=(width, height)`
and an RGBA `background`. Rendering requires a wgpu adapter. A software Vulkan
adapter works without a desktop session.

Pass `camera=Camera(eye=(...), target=(...))` for a custom perspective and
`show_edges=False` to hide surface wires in shaded captures. Pass
`show_isocurves=False` to keep the topological edges and hide interior
surface isocurves. Standalone model curves remain visible.
`lighting="diffuse"` shades the model colors with a directional light and
ambient fill. The default `lighting="viewport"` uses the desktop shading.

Use `model.run("NamedView Save Front detail")` to save the active camera and
pass `view="Front detail"` to capture it.
