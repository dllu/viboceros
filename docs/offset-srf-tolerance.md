# Surface offset tolerance

`OffsetSrf Tolerance` opens a numeric getter with `Loose=No`. It accepts lengths
with units, rejects negatives and retains the current value on Enter. Zero,
`Default`, or a value equal to current document tolerance selects the document
default; later document-tolerance changes are reflected. Other positive values
survive cancellation and document changes. One Escape cancels the whole command.
`Tolerance=value` at the interactive direction prompt also opens the getter;
complete command/API invocations accept the supplied value directly.

Analytic offsets remain exact. General native offsets use the requested upper
bound for kernel accuracy and NURBS fitting, retaining the established
conservative fitting cap for compatible surface/trim geometry. Coarse requests
can therefore produce more accurate results than requested. `Loose=Yes`
hides this option and keeps its original-basis interpolation behavior.

Three licensed Rhino 8 captures ran on private Xvfb displays. All 30 outcomes
remain, including two input-driver diagnostics. The 28 qualified cases cover
default/custom memory, unit conversion, inline getters, negatives, Enter, Escape,
postselection, free-form and rational surfaces, curved trims, walls and two-sided
skins. Portable builds atomically decline seven general non-Loose cases. Native
builds also decline the nonuniform rational and curved-trim captures because the
kernel exports parameters outside their NURBS domains; their geometry parity remains pending.
Command and UI replays compare geometry, metadata, selection, identity, previews,
actual Undo/Redo and 94 3DM round trips across both build configurations.

Finite trimmed-face and boundary comparisons use `1e-6` for planes and
`max(1e-5, 2*requested)` for curved refits, in model units (millimeters in these
captures). Rhino's fitted output can exceed its requested tolerance at sampled
ideal-offset points. Product residuals at independently displaced source-normal
stations must remain within the requested value plus `1e-10`; native residuals
are recorded separately. These station checks do not certify a continuous
true-offset bound, complete native parity, arbitrary singularities, collapse,
self-intersection, requests finer than `1e-7`, or comparative performance.

[Qualification record](offset-srf-tolerance-validation.json) ·
[Reproduce checks](../tools/rhino_oracle/check_offset_srf_tolerance.py)
