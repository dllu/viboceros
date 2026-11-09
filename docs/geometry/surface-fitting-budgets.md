# Experimental surface fitting refinement budgets

[Automatic UV fitting](surface-fitting-automatic.md) · [Image fitting](surface-fitting-images.md)

`SurfaceFitRefinementBudget::try_from_surface(source, degree)` constructs an
opt-in candidate-complexity policy from a clamped, continuous surface whose
degrees equal the requested degrees. Unsupported basis relationships return
`None`; invalid fitting inputs return an error. Inputs remain unchanged.

For each axis, the ceiling is `degree + number of nonzero active knot spans`.
Repeated interior knots do not increase it. The initial one-patch control count
is `degree+1`; this proposal is always eligible. A larger proposal must stay
within both ceilings and reduce at least one axis below its ceiling. One axis
cannot spend unused capacity from the other axis. Getters expose the initial
counts and ceilings; `admits_control_count` evaluates a proposed pair.

This experimental policy reflects equal-degree public Rhino observations.
It is neither a geometry certificate nor a predictor of fitting success or
null results. Eligibility says nothing about a candidate's accuracy. Existing
bounded fitters retain their current refinement and admission behavior; they
do not apply this policy. Fitting work/control limits remain separate.

The local JSON operation `surface_fit_refinement_budget` accepts `surface`,
`degree_u`, `degree_v` and optional `candidate_controls`. It returns `supported`,
`initial_controls`, `maximum_controls` and `candidate_admitted`. Unsupported
bases produce null counts and null candidate eligibility; an omitted candidate
also produces null eligibility. Unknown is distinct from a rejected candidate.

## Evidence and remaining work

Two new headless capture families isolate rational Bezier and piecewise
polynomial tensors, repeated versus distinct knots, unequal axes and degrees
1 through 4. They contain 57 public `Surface.Fit` calls: 18 surfaces and 39
nulls. Every input receives complete authored-input and unrefined-source
equivalence bounds at `1e-9`; source immutability is recorded. Successful
outputs reproduce at that same complete bound when their **captured bases**
are supplied to the independent polynomial fitter, including 3DM round-trips.

Equal input control counts can yield different native outcomes. A rational
Bezier refined to 7×7 with one tripled knot remains null at `0.01`; three
distinct inserted knots per axis produce a native 5×5 fit. A polynomial source
with 12×12 controls similarly remains null with repeated insertion but yields
7×7 after distinct insertion. Adding capacity to just one axis can still fail.

Degree changes do not follow this policy; those observations remain unknown.
The budget also does not choose knot locations, certify source accuracy, or
implement Rhino's complete stopping/null policy. Native adaptive basis selection
and the physical `FitSrf` command remain unfinished. The
[validation record](../surface-fit-capacity-validation.json) preserves both
successful captures and all nulls, together with portable/native replay logs.
