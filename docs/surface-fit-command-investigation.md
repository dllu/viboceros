# Physical FitSrf observations

[Surface fitting](geometry/surface-fitting.md) · [Image bounds](geometry/surface-planar-images.md)

The `surface_fit_command` probe records complete command events, inactive
completion, object definitions/B-reps, selection, attributes and Undo/Redo in
an empty owned document. All launches used `tools/rhino_oracle/run_headless.sh`
with private Xvfb and runtime directories; no desktop windows were opened.
The probe uses public Rhino APIs and command output only.

## Fifteen captured workflows

| Outcome | Cases | Observation |
| --- | ---: | --- |
| Refit | 9 | Command definitions equal public `Surface.Fit` definitions; complete UV bound is zero. |
| Selection rejection | 3 | `AddSurface` inserted creased sources as four-face B-reps; FitSrf left them unchanged. |
| Fitting failure | 3 | Explicit single-face inputs returned SDK null and command history reported failure to simplify; inputs survived both DeleteInput settings. |

The eight-case `surface_fit_commands` family varies planar, polynomial and
rational sources, source redundancy, degrees, zero tolerance, preselection,
DeleteInput and OutputLayer. The seven-case `surface_fit_command_faces` family
forces one face with `AddBrep(..., splitKinkySurfaces=False)` to distinguish
selection rejection from fitting failure. Dense polynomial cases then refit;
the original polynomial and two original rational cases still decline.
The frozen case named `polynomial-loose-single-face` actually contains a plane
at tolerance 1; the generator and test explicitly preserve this historical label.

All 15 commands completed exactly once, with no active command remaining.
Undo restored the before snapshot and Redo restored the command result.
All resulting objects were valid and untrimmed.

## Observed policies and limits

Successful DeleteInput replacements retain the input identity, name, object
color, attribute user text and groups; OutputLayer=Current moves the replacement
to that layer. Preselected replacements remain selected; postselected ones do
not. Copies to Input preserve these attributes and groups, with the original
retained. The captured copy to Current has default attributes and no groups.
Geometry user-data preservation is not qualified by this probe.

The captured zero-tolerance command uses document absolute tolerance `0.001`,
consistent with the public [FitSrf documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/fitsrf.htm).
Every case uses ReTrim=No; trimmed inputs, option memory, interactive getters,
general adaptive fitting policy and broader metadata combinations remain open.
The Rust `FitSrf` command is still unregistered. The bounded fitting APIs and
planar image certificate are independent capabilities, not command parity.

Two initial probe failures incorrectly assumed a single-face object snapshot.
Both failed launches retain exact producer closures and error capsules. The
[validation record](surface-planar-image-validation.json) audits those failures,
both successful families, full-image comparisons and portable/native replay logs.
