# Headless Python modeling

Build Viboceros using the [repository setup instructions](../README.md#build-and-run)
and run your script from the checkout with Python 3.10 or newer. To import it
from elsewhere, install the Python package with `python -m pip install -e .`
and put the built `viboceros` executable on `PATH` or set `VIBOCEROS_BIN` to its
path. `Session(executable=...)` can select a specific build, including one built
with `--features native-smlib`.

```python
from viboceros import ENTER, Pick, Session

with Session() as model:
    body, tool = model.run(
        "Box 0,0,0 38,25,0 12",
        "Box 25,8,-2 42,17,-2 16",
    ).added
    cut = model.run("BooleanDifference", Pick(body), ENTER, Pick(tool), ENTER)
    part, = cut.added
    model.run('SetObjectName "Housing"', select=[part])
    model.select()
    model.save("housing.3dm")
    model.export("housing.step")
    for view in ("Perspective", "Top", "Right"):
        model.screenshot(f"housing-{view.lower()}.png", view=view)
```

A session owns a document and all of its command, selection, viewport, and undo
state. Use `run()` with any supported command line, one line per argument. The
[command reference](commands/README.md) describes each command's arguments.
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
  references after edits that change topology. `grips(obj)` lists the enabled
  control or edit points. Use `select_grips(*grips)` or `Pick(grip)`.
- `PickPoint((x, y, z), view="Top")` projects a point and uses the viewport's
  usual hit testing. `ActivateView`, `Key`, and `Pointer` cover view activation,
  modifier keys, and screen-space clicks or drags. `viewports()` gives each
  viewport's rectangle and `project()` maps a world point to a screen position.
- Results carry the console output in `messages` and the added, deleted, and
  geometry-modified object IDs. Pass `track_changes=False` to omit the object
  comparison for large batches. A reported error raises `CommandError`, with
  the partial result on the exception's `result` attribute. Pass `check=False`
  to keep submitting steps after errors. Earlier edits remain available to Undo.

## Files and images

Use `save()` for the current project or `save(path)` for Save As. `open(path)`
replaces the document with a 3DM project. `import_file(path)` and `export(path)`
choose 3DM, STL, or STEP by extension. Pass `native_step=True` to use the native
STEP option on a compatible build, or `ascii_stl=True` to write text STL.

`model.screenshot()` writes PNG using the desktop viewport's renderer. Give it a
viewport name or index, a saved named view, or a standard direction including
`Perspective`, `Top`, `Bottom`, `Front`, `Back`, `Right`, and `Left`. It uses
shaded mode and fits visible geometry by default. Set `mode="Wireframe"` or
`mode="Ghosted"`, `fit=False` to keep the camera, or supply `size=(width, height)`
and an RGBA `background`. Rendering requires a wgpu adapter. A software Vulkan
adapter works without a desktop session.
