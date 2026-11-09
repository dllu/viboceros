# Automatic bounded surface fitting

[Explicit-basis fitting](surface-fitting.md) · [Surface bounds](surface-correspondence.md)

[Image fitting](surface-fitting-images.md) separately permits UV speed changes
under a complete geometric bound. This API retains normalized UV accuracy.
The opt-in [refinement budget](surface-fitting-budgets.md) is instrumentation
for proposal complexity; it does not change this fitter's behavior.

`try_fit_surface_with_bound(source, options, limit)` chooses a polynomial U/V
basis and returns a `BoundedSurfaceFit` only after certifying the complete
original surface. `SurfaceFitOptions` specifies degrees and control ceilings;
defaults are cubic with at most 64 controls in each direction. The source UV
domains remain unchanged. Zero requires exact agreement.

Fitting starts with one Bezier patch. Residual variation selects knot intervals
for midpoint insertion, prioritizing larger errors when a control ceiling is
near. Degree-plus-one differences help avoid refining a direction already
represented by its polynomial basis. Source evaluations share a cache, capped
at 262,144 points. Simple output knots retain C(degree-1) joins.

Samples guide proposals. Acceptance requires the exact full tensor bound,
including source interiors and one-sided knot limits. If the bound fails and
samples find no error, available intervals are refined to expose missed
features. A regression uses a degree-12 excursion that vanishes at every initial
interpolation and refinement station.

`None` means no candidate was certified within the output, sampling and
arithmetic limits. It does not prove that fitting is impossible. Invalid inputs
or solver errors return an error. Source geometry remains unchanged, including
when a later proposal fails. Existing source limits are degrees at most 16,
4096 controls, and nonzero common-sign weights. Target degrees are 1..11.
Trimming, tangent continuity and topology are outside the certificate.

`try_fit_surface_with_bound_with_solver` shares selection and admission with
external solvers. `viboceros_smlib::fit_surface_with_bound` uses usd-brep's
batched collocation through the existing Cartesian layout adapter.

## Python/JSON

The `surface_fit_bounded` operation accepts `surface`, `degree_u`, `degree_v`,
optional `maximum_control_count` (default `[64,64]`) and `absolute_tolerance`
(default request tolerance). It returns the fitted definition and complete
maximum displacement, or null fields when declined, plus unchanged before/after
source definitions. Zero is strict in this protocol.

## Native comparison limits

The [evidence record](../surface-fit-adaptive-validation.json) includes 112
public `Surface.Fit` observations. Automatic bounded fitting is compared with
every case using `[16,16]` control ceilings. Source accuracy and native output
agreement are recorded separately. The complete native comparison gate remains
`1e-9`; an unproved parameter correspondence is not proof of geometric inequality.

Two new capture families isolate source redundancy, degrees, U/V strips and
planar rational parameterizations. Exact source refinement can change native
success, control counts and knots. All 16 paired inputs receive complete
source-equivalence checks. Supplying captured bases separately tests control
production; those results do not establish automatic selection parity.

At the tested ceilings, each backend certifies 87 outputs, including 35 complete
native matches. The ledger also retains 40 unproved native correspondences,
12 certified outputs where native returns null, 15 local declines with native
surfaces, and 10 shared declines. All 90 successful native outputs reproduce
when their captured bases are supplied separately. These are different claims.

Rhino's adaptive knot selection, physical `FitSrf` options and retrimming remain
unfinished. The `FitSrf` command is still unregistered.
The [physical command investigation](../surface-fit-command-investigation.md)
records selection eligibility and nine SDK-equal untrimmed refits separately.

A [later image comparison](../surface-image-fit-validation.json) certifies 48
native image matches for these same 87 UV-bounded outputs, resolving 13 of the
40 unproved parameter correspondences. The 35 UV matches remain a separate claim.
