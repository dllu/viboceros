# Projected curve locations

`NurbsCurve::closest_parameter_in_projection` locates a bounded curve point
through a caller-supplied 2D projection. Viewport edge and curve picks supply
the precise camera projection; endpoint snaps and feature selection remain
viewport policy. A perspective pick minimizes screen distance, which can
choose a different station from world-space distance to the viewing line.

The query uses 16 sampling intervals per nonempty knot span and refines eight
best neighborhoods in the native domain. It retains eight intervals with nine
allocated slots, avoiding storage and full sorting of every sampled interval.
Nonfinite targets, wholly invisible curves and more than 4,096 spans return
no location. This is a bounded numerical locator, not a certified minimum.

[Validation](line-pick-context-validation.json) records 14 native fixed Tangent
workflows across Top, Front and Perspective: circles, tilted circles, an arc,
a spatial cubic, signed lengths, BothSides and Planar. Public `PickFrustumTest`
points corroborate the native fixed contacts. Rust locates those contacts on
independently prescribed curves through the recorded pre-click pick transforms;
expected points and output geometry do not seed the search.

Both builds replay the commands and application getters, compare coefficients,
weights, knots, domains, attributes and 33 local samples, and round-trip each
construction through 3DM. Windowless edge-pick events retain snap, visibility,
cache and constrained-click coverage. The earlier fixed perspective replay
[diagnostic](line-tangent-rays-validation.json) used the world-space metric;
its native output and input ray are identical to the corrected capture.

Run `python3 -m tools.rhino_oracle.check_line_pick_context` for geometry limits,
both builds and shared regressions. Native generators use private Xvfb sessions.
Arbitrary native camera replication, ambiguous depth ranking, eye-plane crossings,
remote coordinates, singular curves, continuous distance and comparative speed
remain unqualified. Earlier local tangent-search failures remain unresolved.
