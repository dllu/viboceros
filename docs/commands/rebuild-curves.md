# Rebuild curves

[Command reference](README.md) · [Surface Rebuild](rebuild-surfaces.md)

Bare `Rebuild`, `RebuildCrv` or `RebuildCurve` opens a readonly curve preview
after preselection or object picking. Edit `PointCount`, `Degree`,
`PreserveTangents`, `DeleteInput` and `OutputLayer`; Enter accepts the prepared
curves in one Undo step. `Preview` retains the current cached scene. Cancel or
Escape drops the preview without changing document geometry or history.

```text
Rebuild
PointCount 8
Degree=3 PreserveTangents=Yes
DeleteInput=No
```

Options accept `Name=value`, `Name value`, or an option name followed by its
value at the next prompt. Enter at a value prompt keeps the current value.
`Points` and `PreserveEndTangents` are aliases. Fresh invocations use 10 points,
degree 3, no tangent preservation, deletion and input-object output layer.
Curve option memory and native dialog parity remain unverified.

Compact buttons beside the command line show the current count, degree,
tangent, deletion and layer options. Numeric buttons open the corresponding
typed value prompt; boolean and layer buttons toggle their current values.
`Preview`, `Accept` and `Cancel` use the same handlers as typed input. Accept is
disabled while preparation has failed, and button actions return keyboard focus
to the command field.

Complete inline commands continue to accept directly on preselected curves,
or after picking when started without selection:

```text
Rebuild PointCount=8 Degree=3 PreserveTangents=Yes DeleteInput=No OutputLayer=Current
```

Requested counts accept 1 through 1,000, degrees 1 through 11. The existing
kernel raises a small count to its degree/topology minimum. Open outputs use
clamped non-rational uniform knots; closed outputs use periodic interpolation
and repeated controls. This is an approximation, without a continuous bound
between the original and rebuilt loci. Tangent preservation applies to eligible
open curves. The new preview uses the existing kernel unchanged.

Changing deletion or output layer reuses prepared geometry. Count, degree or
tangent changes rebuild it. Invalid syntax keeps the previous preview; valid
options that fail geometry preparation clear the scene and block acceptance
until an edit recovers. Source geometry, attributes, group/root text, selection,
tolerance and current layer are checked before every edit or acceptance.
Unrelated document/display changes refresh the background using prepared curves.

Deleting on the input-object layer replaces geometry in place, retaining identity
and attributes. Copying or output on the current layer creates objects with
default attributes on the chosen layer, following the preceding command policy.
Undo and Redo restore the source and accepted result; preparing or cancelling
retains an existing redo branch. Mixed selections retain the preceding routing:
selected surfaces choose the surface Rebuild branch.

The [source hashes](../curve-rebuild-preview-provenance.json) and command/app
regressions cover readonly preparation, exact prepared-output acceptance, option
reuse, closed curves, failure recovery, picking, cancellation and stale-source
rejection. Additional egui click tests cover the visible controls, disabled
acceptance, surface U/V independence and source validation before button actions.
Production Xvfb inspection was attempted but could not open an X11 display;
the sandbox prevents Xvfb from binding listening sockets. No completed production
pixel capture or fresh Rhino comparison is claimed. Native geometry, selection
and option-memory parity need further audit.
