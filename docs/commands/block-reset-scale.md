# BlockResetScale

`BlockResetScale [Mode=One|Automatic]` changes selected block-root placements
without modifying shared definitions. Bare input uses the remembered mode, which
starts as One. Eligible preselection executes immediately. Command-first input
uses the block getter; type Mode, then One or Automatic, or use `Mode=value`
while selecting. Enter accepts, and Cancel or Esc discards the model edit.
Accepted mode choices survive cancellation, Undo and later documents in the
same registry. Independent registries start with their own defaults.

One targets unit scale. Automatic applies the first matching pair's scale when
two axis magnitudes are close; otherwise it uses their arithmetic mean. The
closeness predicate uses absolute and relative bands calibrated against native
boundary decisions. This is a behavioral calibration, not a claim about Rhino's
proprietary implementation. The placement's orthonormal instance frame comes
from its first axis and the oriented
plane normal. Scaling acts in that frame. Reflections and insertion coordinates
are retained. Sheared placements follow the native frame-scaled result; their
final matrix columns need not all have unit length in One mode.

Root IDs, attributes, attribute text, geometry text and model groups survive.
Catalog geometry and nested references remain unchanged. Sources must be
editable block roots; ordinary selected objects are excluded by the command
getter. Preparation stages every geometry before committing, so missing,
protected, invalid or unrepresentable sources cannot cause partial replacement.
One operation creates one Undo entry and replays the original geometry snapshots.

The document API is `reset_block_scale` with `BlockScaleResetMode`. It uses
compensated projection/composition, robust oriented normals and a rational
rescaling fallback when a floating-point ratio overflows or underflows but the
final coefficients remain finite. Local tests constrain extreme scales,
metadata, group/catalog preservation, rollback and history. The native workflow
oracle adds `reset_scale`, including preselected and cancelled-choice probes.

77 native workflows match all 388 recorded states. The calibrated band is
`sqrt(machine epsilon) + 2.25e-10 * (abs(a) + abs(b))`; this matches the tested
comparisons across three scale magnitudes, adjacent doubles and translated roots.
It is sampled evidence, with wider numeric qualification still open. A native
trial at insertion coordinates `1e16` rejected instance admission before reset;
that failed setup is not counted as a reset comparison.

See [source/capture provenance](../block-reset-scale-provenance.json) and the
[validation checkpoint](../validation-checkpoint.md). Linked definitions,
native mouse/group picking, selection/history parity, GPU rendering and
performance remain unverified or unfinished. The general project still requires
broader Rhino compatibility work.

Reference: [BlockResetScale](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#BlockResetScale).
