# Divide input and preview

Bare `Divide` or `Div` uses the shared curve getter, then a typed numeric/options
stage. Eligible preselected curves start at the numeric stage. Noncurve
preselection is excluded, with the original selection restored on cancellation.
Complete argument lines still execute directly, or execute after curve picking
when no eligible sources are selected.

Count, Length and EqualChordLength edits retain separate values for the current
invocation. Boolean output options accept explicit Yes/No settings or a bare
name to toggle. Numeric and option answers update the pending result; a later
Enter accepts it. Invalid values and duplicate settings leave the prior prompt
and preview unchanged. The local initial count and both distances are 1; saved
Rhino preference/default memory is not implemented by this checkpoint.

The preview uses the same read-only output preparation as command execution.
Point mode shows generated point locations. Split mode shows piece boundaries,
including retained or deleted remainder behavior. Immutable source geometry
snapshots, source IDs/order and tolerance qualify the cached markers, which are
shared with the existing viewport point overlay. Rendering does not repeat
division calculations. Source changes hide stale markers and reject acceptance;
a replacement command or Escape removes the prompt and preview. Preview edits
create no document objects, groups or Undo entries; accepted output forms one
command transaction.

The workflow follows the documented [Rhino Divide interaction](https://docs.mcneel.com/rhino/8/help/en-us/commands/divide.htm).
Local regressions cover preselection, command-first picking, count/length/chord
edits, remainder boundaries, invalid edits, cancellation, source invalidation,
replacement commands and one-step Undo. Existing licensed Rhino captures are
replayed for output geometry and metadata. These tests do not establish native
preview styling, closed-curve seam picking, direction reversal, SubCrv input,
persistent defaults or full interactive parity.

[Provenance](divide-input-provenance.json) records final sources and validation.
