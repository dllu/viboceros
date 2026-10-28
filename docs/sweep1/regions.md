# Section region correspondence

Sweep1 matches independent profile regions before preparing the common NURBS
basis. For example, two segments in one profile can correspond to three in
another: the existing first corner stays aligned, and the shorter profile gains
the extra subdivision within its second segment. A union of independently
normalized knot vectors would give a different surface between those profiles.

The shared Loft region matcher supplies this correspondence. Sweep1 retains its
own common weight scaling, rail placement, and blending policies. Real profiles
are matched before constructing any point-end poles. Matching does not reorder
profiles, reverse authored curves, or relocate seams.

Sixteen frozen Rhino SDK and actual command cases cover unequal corner counts,
different source domains, equivalent cubic Bezier segments, equal corner counts,
retained/refitted rails, and one or two point ends. Both backends certify the
complete surface images within `1e-7`, including every split-face join.

The comparison accounts for reversed rail direction and charges small U-knot
differences using complete curve bounds on every tensor column. It also covers
interior profile rows, rational weight coupling, and both limits at knots.

The [validation record](regions-validation.json) contains the measured bounds,
checks, and source inventory. Section Refit by tolerance remains in development;
the broader [Sweep1 limitations](validation.md) still apply.
