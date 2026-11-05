# UseExtrusions

`UseExtrusions [Extrusion|Polysurface]` controls future eligible creator outputs.
The default is `Extrusion`. In the application, the bare command prompts for a
choice; Enter keeps the current mode, Escape cancels, and another command
replaces the prompt. The command API returns the current choice on bare input.

The setting belongs to the command registry and applies across its documents.
It leaves existing geometry, selection, Undo and pending Redo intact. Independent
registries start with their own default.

The policy covers `Box`, `Cylinder`/`Cyl`, perpendicular planar `ExtrudeCrv`,
`Tube`, and constant-radius `Pipe` on straight rails. NURBS lines and two-point
polylines qualify. Extrusions retain the rail parameter interval; an open thick
pipe's second wall uses a distance-based interval, matching native behavior. Flat-capped thick pipes retain an inner extrusion profile;
open thick pipes create two separate walls. Explicit radius stations always
produce B-reps, including constant-radius stations. Single-wall round caps, tapered and
curved rails, and oblique or nonplanar curve extrusions use explicit geometry.
`ExtrudeSrf` retains B-rep output in both modes and preserves whole-object
preselection when keeping inputs. It requires the native kernel; default builds
check that rejection leaves the document unchanged.

Native public-command captures cover eight preference workflows and 162 creator
workflows, ten Pipe station/domain cases, and a 16-case pilot. Tests check output types, trimmed boundaries,
surface geometry within `1e-6`, selection, source preservation and Undo/Redo.
Non-Box extrusion surfaces also compare full controls, weights and knots.
Rhino's explicit constructors can use different face order, parameter axes,
degrees and cap support rectangles. Exact explicit parameterization, arbitrary
inputs, continuous error bounds and comparative performance remain unqualified.
Successful observations retain complete geometry and frozen producer sources.
The former short-span ExtrudeCrv regression is resolved; see the
[geometry and getter follow-up](../short-span-extrusions.md).

```sh
cargo test -p viboceros-command --release --test use_extrusions_native
cargo test -p viboceros-oracle --test use_extrusions
cargo test -p viboceros use_extrusions
python3 -m unittest tools.rhino_oracle.test_use_extrusions
```

All licensed captures use `tools/rhino_oracle/run_headless.sh` with private Xvfb
and runtime directories. See [extrusion storage](../extrusions.md) and
[creator qualification](../creator-extrusions.md).
