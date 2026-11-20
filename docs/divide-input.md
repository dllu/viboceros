# Divide input and preview

Bare `Divide` or `Div` uses the shared curve getter. With closed inputs,
drag the seam markers, use `Flip` to reverse a marker, `Automatic` to align
the closed seams, or `Natural` to restore the original seams. In typed
input, `SelID` chooses the seam and a model point or `Parameter=<t>` places
it. Enter continues to numeric options. Open curves go straight to those
options. The original preselection is restored on cancellation when Divide
temporarily excludes noncurve objects. Complete argument lines still execute
directly or after curve picking.

Type `SubCrv` at curve selection or start with `Divide SubCrv`. Pick a curve
or surface edge, then its temporary start and end. Typed points work on the
chosen source. To enter a length, type it after the start and use the cursor
or a typed point to confirm the direction. `Direction=Locked` holds the
current side and `Direction=Free` releases it. Escape leaves the portion
getter and returns to source selection. Accepted portions stay highlighted
and can be mixed with whole-curve picks. `SelNone` clears both kinds of input.

Count is remembered separately from a shared Length/EqualChordLength distance.
New invocations recall [session preferences](divide-preferences.md) in count mode.
Boolean output options accept explicit Yes/No settings or a bare
name to toggle. Numeric and option answers update the pending result; a later
Enter accepts it. Invalid values and duplicate settings leave the prior prompt
and preview unchanged. The initial local count and distance are 1. Valid edits
retain session settings through cancellation and Undo.

In Length mode, clicking a selected curve or typing a point on it toggles
the measuring direction for that curve. Reversal updates the station markers
immediately. The selection and curve geometry stay intact during both getters.

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
Regressions cover preselection, command-first picking, viewport seam drags,
individual length reversal, count/length/chord edits, remainder boundaries,
invalid edits, cancellation, source invalidation and one-step Undo. In private
headless Rhino 8.32.26160.13001 runs, dragging a radius-5 circle's seam from
(5,0) to (0,5) started a four-part division at the new seam. Flipping the relocated
seam placed the count-mode seam point last, measured length clockwise and
emitted splits in increasing shifted parameter order. Picking a length-10
line in Length mode with a step of 3 yielded stations at 1, 4, 7 and 10.
Split preserved the increasing source intervals and omitted the leading
partial piece with either remainder setting. EqualChordLength used the
relocated seam and kept the curve's natural direction. Native SubCrv
captures covered repeated portions of a single curve, ordered whole and
partial inputs, reversed split intervals, surface-edge input, and numeric
length wrapping around a closed seam. Only whole-curve picks responded to
Length direction flips. Current coverage includes geometry and interaction.
Native preview styling and disk-persisted defaults remain open.

[Numeric-prompt provenance](divide-input-provenance.json) records the initial
prompt implementation and validation.
