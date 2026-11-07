# Live BlendCrv curvature display

[Command reference](commands/blend-curve.md) · [Validation record](blend-curve-display-validation.json)

`DisplayCurvature` enables combs on the captured sources and the live blend
candidate. It is idempotent. Temporary portions draw their own curve, rather
than enabling their original parent. Surface references use the supporting
objects' curvature graphs. The Curvature Graph panel shares scale, density,
direction and color settings with ordinary document graphs. Closing the panel
hides the temporary display; the option enables it again.

Existing graph analysis activates a new blend's display automatically. Acceptance
and cancellation remove the temporary analysis and preserve the original graph
sources. A quoted `'_CurvatureGraphOff` does not end the getter or disable its
temporary analysis, matching the captured Native behavior. Graph operations create
no objects or undo entries. The completed blend remains one geometry edit.

Continuity defaults initially use G2/G2. Options entered before picking are local
to that invocation. Changes made in the ready preview persist per endpoint,
including after cancellation; a free point's forced G0 remains temporary. Separate
Native sequences exercise both preselection choices and persistent preview controls.

Combs reuse the qualified curvature sampler and usd-brep extraction. Differential
samples are cached across scale/color changes and rebuilt for new geometry or
sampling settings. Hover candidates have their own cache; cancel restores the
accepted preview. The shared painter displays hairs and strip outlines in wireframe,
shaded and ghosted modes. Sampling is bounded at 200,000 stations.

Six immutable captures contain 27 observations. One pilot accepted the blend but
left an extra Enter unconsumed; it is a diagnostic, followed by a complete repeat.
The 26 complete workflows contain 22 acceptances, four cancellations, 50 physical
picks, 726 independent point/tangent stations, 372 derivative stations, 3,762 public
curvature samples and 60 viewport images. Public analysis-mode queries check 57
of those images. An overlapping unrelated curve opened a selection menu in a
separate attempt; the owned process was interrupted with no published observations,
and all recipes were recaptured with disjoint picks and explicit initial display state.

Portable and native command/UI replays check geometry, curvature vectors, scaled
tips, graph lifetimes, identities, attributes, selection, undo/redo and 3DM round
trips. Windowless paint tests cover four views in three display modes. Model and
curvature/tip gates are 1e-6; Native approximate domains retain their separate gate.
The captured corpus's maximum geometry and scaled-tip differences are 7.59e-15
and 2.04e-13, respectively.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_display
```

Rhino captures use private Xvfb displays; application tests are windowless. Bitmap
evidence qualifies the observed composition and lifecycle, not pixel equality.
Full Native panel mouse trajectories, arbitrary occlusion, singular extremes,
continuous distance and comparative performance remain unqualified. Existing
mathematical G3/G4 and approximate-domain limits remain.
