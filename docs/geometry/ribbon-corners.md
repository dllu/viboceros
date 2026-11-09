# Ribbon corner correspondence

[Polyline ribbon API](polyline-ribbons.md) · [Validation record](../ribbon-corner-validation.json)

A Sharp outside join extends its neighboring offset segments. When an adjacent
inside corner clips one of those segments, the source split follows the
extended segment's original affine parameter. Other inside corners retain the
direct miter correspondence. A Sharp closing seam does not initiate adjacent
inside-corner splits. When both incident strips were extended at
other corners, their clipped seam intervals omit extra inside fan faces.

This accounts for the previous zigzag discrepancy without selecting fixture
IDs or copying native control nets. Concave loops, moved seams, unequal source
charts, tilted planes and longer mixed-turn chains exercise the rule.

Smooth outside fans are polynomial cubics. Given offset endpoints A/B and
oriented unit tangents T0/T1, both handles have length
`|B-A| * (3 - dot(T0,T1)) / 6`. The four controls are A, A+hT0, B-hT1, B.
Independent native angle holdouts range from 15 to 165 degrees and include
changed distances, both sides and native PolylineCurve inputs.

Each patch carries its source/offset corner identities through edge assembly.
Only declared identities can share a vertex, including isolated point contacts.
Every member must agree with the stored representative under kernel coincidence
and model tolerance; transitive chains cannot drift beyond that bound. Surfaces,
edge curves and UV trims are retained. Unrelated coincident tips stay separate.

The replay compares complete tensors at `1e-9` with full rectangular trim-domain
coverage, injective edge/vertex matching and face-loop edge/type incidence.
Both builds qualify all 110 Ribbon workflows across five capture families.
Support-surface 3DM round-trips retain topology counts. Samples do not admit
comparisons. The archived public Offset workflows inform the construction;
local Offset command parity is not qualified by this replay.

The earlier [polyline record](../ribbon-polyline-validation.json) remains frozen.
The new record supersedes its Sharp zigzag and None point-incidence gaps.
Native face reversal flags are recorded separately; loop winding, output charts,
object order, self-intersection trimming and full Ribbon command parity remain
unqualified.
