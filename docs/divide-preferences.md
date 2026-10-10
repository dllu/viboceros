# Divide command preferences

Divide now remembers count, one shared Length/EqualChordLength distance, and
MarkEnds, Split, DeleteRemainder and GroupOutput settings for the command
registry's lifetime. `Div` uses the same settings. New invocations start in
count mode; choosing Length or EqualChordLength recalls the shared distance.
Independent registries start with their own defaults. Preferences are outside
document history and survive Undo, changing documents and canceled UI input.

Valid interactive edits are saved after their geometry preview succeeds.
Invalid numbers, duplicate options and failed previews leave preferences and
the pending result unchanged. Complete argument commands inherit omitted flags
and save their settings after successful execution. The static `Options::parse`
API remains deterministic with initial defaults for fixture validation.

## Native evidence

The licensed Rhino 8.32.26160.13001 ran in a dedicated Xvfb display with private
scheme `VibocerosOracleDividePreferencesQualified20261010`. Eight fixed recipes
contain 30 public Divide invocations on fresh 20.7-unit lines. The helper records
EndCommand success/cancel events, command history, output coordinates, groups,
source retention and 17 samples per split curve. It requires an idle Rhino,
empty owned document, private settings scheme and one iteration.

The qualified capture establishes:

- A saved count of 6 repeats as 6 after count, Length or EqualChordLength use.
- Length 2.5 followed by EqualChordLength 3 makes the next Length default 3.
- MarkEnds and GroupOutput changes followed by Cancel affect the next command.
- Split followed by Cancel makes the next count invocation produce curve pieces.
- Canceling a DeleteRemainder change retains the new remainder policy.
- Entering a mode then canceling leaves the stored distance available, while the
  next invocation still starts in count mode.

The UI replay compares all 30 invocations' point/curve positions within 1e-9,
group membership counts, output counts and source retention. Split domains are
recorded but excluded from this replay: local lines and native lines can use
different parameterizations. Separate command/kernel fixtures qualify domains
for their corresponding source representations.

## Limits

Rhino finishes when a numeric response is entered; Viboceros updates a preview
and waits for Enter. Locally, a valid numeric preview also updates remembered
numbers even if later canceled; the native capture does not isolate cancellation
between numeric entry and acceptance. Initial local flags remain No; the fresh
native scheme showed MarkEnds=Yes. Disk-persisted preferences, native factory
profile parity, direction/seam picking, SubCrv selection and preview appearance
remain open.

An initial six-case capture is retained as a diagnostic. Its canceled Split
recipe placed GroupOutput after Split=Yes, where Rhino did not offer that option;
the qualified eight-case capture corrects the order. Raw captures remain
unchanged. Source snapshots and [provenance](divide-preferences-provenance.json)
separate the capture helper from the final implementation.
