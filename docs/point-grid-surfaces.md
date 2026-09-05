# Point-grid surfaces

[Surface commands](commands/surfaces.md) · [Oracle](oracle.md)

`SrfPtGrid` constructs a surface through an ordered rectangular point grid.
`SrfControlPtGrid` instead uses the supplied locations as its control net.
Both commands create one unselected B-rep with fresh attributes on the current
layer. Existing objects and selection are retained. `KeepPoints=Yes` also creates
one unselected point cloud in the original entry order, not separate point objects.
Creation, including that cloud, is one atomic undo/redo transaction.

## Input and degrees

The first count is U and the second is V. Enter all V stations for U=0, then all
V stations for U=1, and so on. This is Rhino's command/API ordering. The Rust
constructors consistently accept **U-fast** data (`points[v * count_u + u]`);
the command and oracle adapters transpose at their boundaries.

```text
SrfPtGrid DegreeU=1 2 DegreeV=1 3 0,0,0 0,1,1 0,2,0 3,0,0 3,1,2 3,2,0
SrfControlPtGrid Degree=1 2 Degree=1 3 0,0,0 0,1,1 0,2,0 3,0,0 3,1,2 3,2,0
```

Native command degrees default to three, with open directions and
`KeepPoints=No`. `SrfControlPtGrid` uses `Degree` separately before each count;
`SrfPtGrid` uses the distinct `DegreeU` and `DegreeV` options.
Degrees must be 1–11 and each command count must exceed its
requested degree. `ClosedU` and `ClosedV` apply only to `SrfPtGrid`; closed
directions require at least three stations. Complete coordinates are required;
incremental viewport picking and loading a command file are not implemented.

The geometry APIs `NurbsSurface::try_control_point_grid` and
`NurbsSurface::try_through_point_grid` independently clamp degrees to
`1..=min(11, count - 1)`, matching the public RhinoCommon helpers. Their `Brep`
counterparts additionally provide command-level crease topology and orientation.
The command's count prompts are stricter than the public construction APIs.

## Tensor construction

Control grids are non-rational with clamped, unit-spaced knots. Their domains
are `[0, count - degree]` independently in U and V.

Through grids use the mean Euclidean chord length across all opposite-direction
stations for each parameter interval. Open directions use those cumulative
parameters as interpolation sites, with degree-wise averaged interior knots.
Degrees one through eleven are supported, including mixed degrees.

`point_grid/basis` constructs and factors the two collocation matrices once,
using faer full-pivot LU and multiple right-hand sides. The solve is separable
in U and V; it does not construct a dense matrix for the whole tensor grid.
Coordinates are recentered and scaled before solving. Constant coordinate
channels and repeated periodic controls are retained exactly. The final tensor
evaluator is independently checked at every construction site.

## Closed-direction compatibility

Degree-one closure appends the first station to a piecewise-linear direction.
Higher degrees use averaged periodic knots and repeated end controls, but Rhino's
public through-grid operation does **not** use ordinary wrapped chord-parameter
interpolation. It constrains the first independent Greville sites. Some of those
sites lie outside the active domain and use continuation of the nearest boundary
span, not periodic wrapping.

Consequently, **not every input point necessarily lies on the active closed
surface**. For the six-station radius-two cylinder fixture, the first U constraint
is at `-2` while the active U domain is `[0, 12]`. This behavior is explicit in
the Rust API documentation and in oracle records containing construction
parameters, continuation samples, and out-of-domain flags. It is not advertised
as unconditional exact periodic interpolation.

Open directions reject coincident successive complete stations. Higher-degree
closed directions can retain repeated stations when their Greville system is
solvable. Non-finite calculations, singular solves and failed residual checks
return errors; partial geometry is not inserted into the document.

## Topology and numerical limits

The command constructors use the shared tensor-partition B-rep builder to split
genuine creases at a sampled `0.1°` threshold, retaining shared edges, periodic
seams and singular sides. Command oracle records inspect **every face**, sorted
by its native domains, along with orientation and vertex/edge counts.

Closed shells are oriented by signed volume. Since point-grid surfaces are
non-rational, `point_grid/orientation` integrates their polynomial volume flux
using degree-appropriate Gaussian quadrature per knot rectangle. Basis values
and derivatives are reused across the tensor evaluation. Local normalization
avoids world-coordinate cancellation; near-zero sign estimates fall back to the
general B-rep volume integrator. No self-intersection or embedded-solid guarantee
is implied by a signed-volume orientation.

Each input direction is limited to 256 stations; the shared B-rep builder limits
crease partitions to 4,096 faces. Construction checks include local solve error,
world-offset rounding, and the absolute-basis amplification of span continuation.
High-degree extrapolation can substantially amplify floating-point error even
when the active surface agrees closely. Passing degree-7/9/11 cases and
ill-conditioned closed-grid diagnostics have separate fixtures; diagnostic
failures do not weaken the ordinary-grid comparisons.

## Verification

Permanent fixtures cover 53 API cases, 18 command cases, eight higher-degree API
and command cases, four high-degree diagnostics, and four
[same-file 3DM round trips](brep-3dm-interchange.md).
They include reordered tensor storage, degree clamping, mixed degrees, closed
directions, repeated stations, point-cloud retention and full crease topology.
Native tests additionally verify bilinear reproduction, continuation basis
coefficients, exact large translations, `1e-140`/`1e140` scales, and quadrature
exactness through every required polynomial degree.

Fresh isolated Rhino 8 checks passed all 75 ordinary/API/command/interchange
cases at absolute `2e-12` plus relative `1e-14` (maximum difference `3.27e-13`).
The eight passing higher-degree cases use `1e-8` plus relative `1e-12` for full
records (maximum `3.22e-9`), with active samples independently checked at
`2e-12` plus relative `1e-14` (maximum `2.67e-14`).

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_grid.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_grid_high_degree.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-12
```

`point_grid_high_degree_diagnostics.json` is **not a passing full-record
compatibility reference**. It preserves two closed cases through both APIs and
commands:

- Degree nine has an axis collocation condition number about `5.14e7` and first
  tensor continuation amplification about `2.71e6`. Active samples differ by up
  to `1.41e-8`; ordinary epsilon does not apply. At the first construction site,
  native/Rhino double-precision evaluation misses the input by `3.96e-7`/`1.06e-7`.
  Independent 70-digit evaluation of their stored control nets reduces those
  residuals to about `1.16e-9`/`1.72e-8`: most of the apparent residual is
  continuation-evaluation cancellation, not a failed tensor solve.
- Degree eleven has continuation amplification about `1.65e8`; native/Rhino
  construction-site evaluation errors reach `2.79e-8`/`2.66e-8`. Its folded
  shell also exposes an orientation difference: native polynomial integration
  retains positive signed volume, whereas Rhino's command reverses it.
  Independent 70-digit integration of `Z X' Y'` for this separable-X/Y surface
  gives volume `+4.7657446554`. The native orientation is retained; Rhino's flip
  is not treated as a mathematical correctness target for this non-embedded shell.

These are explicit numerical and orientation compatibility limits. They are not
claims of uniform epsilon agreement for every high-degree closed grid.

The reference behaviors were checked through the public
[SrfPtGrid command](https://docs.mcneel.com/rhino/8/help/en-us/commands/srfptgrid.htm),
[SrfControlPtGrid command](https://docs.mcneel.com/rhino/8/help/en-us/commands/srfcontrolptgrid.htm)
and [NurbsSurface APIs](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.nurbssurface).
