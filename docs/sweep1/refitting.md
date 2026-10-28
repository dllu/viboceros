# Section Refit development

This development branch implements `Refit=0.01` and
`Simplify=Refit RefitTolerance=0.01` through the command and JSON adapters.
Rebuild and Refit are mutually exclusive. Real profile copies are fitted before
adding point-end poles, and authored rail stations are retained.

Both backends construct all 86 cases and prove 83 complete Rhino surface images
within `1e-7`. Three fine-profile images still differ from Rhino; their assertions
remain in the acceptance tests. See the
[development record](refit-development-validation.json).
Complete image agreement and command/UI qualification remain outstanding.
The existing qualification records describe `main` before this development
branch; their source inventories have not been refreshed for this prototype.

Section Refit remains in development. Its comparison epsilon is `1e-7`;
successful construction alone does not establish Rhino compatibility.
The native oracle accepts `refit_tolerance` independently of rail refitting and
rejects simultaneous section rebuilding. Frozen captures retain 70 base sweep
cases, 16 controlled cubic sweeps, and 16 public `Curve.Fit` diagnostics. The
public curve fitter produces different outputs from section Refit.

The [controlled inputs](../../tools/rhino_oracle/fixtures/sweep1_refit_common_handles.json)
and [Rhino outputs](../../tools/rhino_oracle/observations/sweep1_refit_common_handles.json)
isolate endpoint-handle fitting. Six cubic profiles share endpoints and tangent
directions, with common handle scales from 0.9 to 1.01. Sixteen SDK recipes use
one profile or two translated copies, at fitting tolerances 0.001 and 0.01.
Producer sources are frozen in the capture's content-addressed archive.

The same profile can produce different fitted boundaries when supplied alone
or as part of a pair. At scale 0.99 and tolerance 0.01, the single-profile output
retains the initial endpoint-frame cubic, while the pair uses adjusted handles.
Independent repeated-fit experiments support a second fitting pass for a single
smooth profile. Straight and polygon profiles retain their separate fitting
path. Every accepted pass has its own complete source-distance certificate;
two passes can accumulate more error against the original profile than the
nominal tolerance. This does not yet explain every adaptive case.

The [public API diagnostic](../../tools/rhino_oracle/observations/sweep1_refit_legacy_diagnostics.json)
compares `SweepOneRail.PerformSweepRefit` with `Brep.CreateFromSweep` on six
single-profile inputs. Each pair has identical complete surface definitions,
including both tight-tolerance source shapes. Choosing the legacy API does not
expose a different intermediate fit on these inputs.

Batch reconstruction samples the original profiles as well as the intermediate
curves, and certifies complete intermediate curve images. It reconstructs each
profile at the shared interpolation stations before checking a fresh complete
bound against its original input. Inserting knots after independent
reconstruction gives different mixed-profile geometry.

Known fitting-span correspondences receive a proposal budget proportional to
their active spline spans, capped at 1024 intervals. Smaller proposals run first.
This removes two closed-fit construction failures caused by the former fixed
budget. Every accepted map still needs the same complete exact image bound;
the nominal tolerance and arithmetic-work limits are unchanged.

A nominal Refit tolerance is not an established final source-distance bound.
The [exact Bernstein regression](../../tools/rhino_oracle/test_sweep1_refit.py) retains a
native 0.001 fit whose boundary is more than 0.0015 from a source point.
