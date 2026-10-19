# ModifyRadius

Select arcs or circles and enter `ModifyRadius`, or start the command, pick the
curves, and press Enter. Enter a positive radius, or choose `Diameter`, `Area`,
or `Circumference` before entering the size. Area and circumference refer to
the complete supporting circle, including when the input is an open arc.

```text
ModifyRadius 5
ModifyRadius Diameter 10
ModifyRadius Area=78.53981633974483
ModifyRadius Circumference=31.41592653589793 Copy=Yes
```

Every eligible curve receives the same radius about its own center. Arc angle
intervals, seams, parameter domains, NURBS control counts/weights/knots, attributes,
geometry user text and group membership follow the existing representation.
For NURBS, control positions change by the uniform scale; the curve is not
rebuilt or converted into an analytic arc. Circular polycurves can be selected
but are unchanged, matching Rhino's command behavior. Lines and other
noncircular curves are ignored.

`Copy=Yes` retains the sources and adds unselected results with fresh attributes
on the current layer. Geometry user text is copied; source groups and attribute
user text are not assigned to the new objects. An unchanged exact radius makes
no copies and preserves redo history.
`RememberCopyOptions` governs the remembered Copy setting. Numeric edits are
prepared before any source or selection changes. Failure preserves model state
and redo history; one successful edit makes one undo step. Command-first
replacements finish unselected. With Copy enabled the picked sources remain
selected on completion and are released when Undo or Redo replays them.

Circular NURBS recognition uses the shared public OpenNURBS predicate at Rhino's
geometric zero tolerance, independently of the document's fitting tolerance.
The portable and `native-smlib` builds share the same command and geometry policy.

See [Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/modifyradius.htm).

The retained 40 native commands cover the four size modes, preselection and
command-first selection, copies, unchanged radii, tilted/reversed arcs, rational
curves, custom domains, batches, mixed selection, circular polycurves and
recognition thresholds. Replays check full NURBS definitions, curve types,
domains and 33 evaluated positions, plus metadata, selection and history. The
comparison epsilon is `1e-8`; this is command compatibility evidence, not a
separate continuous-error certificate or a kernel performance benchmark.

```sh
cargo test -p viboceros-command --test modify_radius
cargo test -p viboceros-command --features native-smlib --test modify_radius
cargo test -p viboceros app::tests::modify_radius
```

See the [validation record](../modify-radius-validation.json) for commands,
results, source hashes and retained logs.
