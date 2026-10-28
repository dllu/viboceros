# Sweep1 point ends

[Command guide](../sweep1.md) · [Construction](construction.md)

Select one or more real profiles and supply either or both point options:

```text
Line 0,0,0 0,0,5
SelLast
SetObjectName Rail
Circle 0,0,2.5 2
SelLast
Sweep1 RailName=Rail Parameters=2.5 StartPoint=0,0,0 EndPoint=0,0,5
```

`StartPoint` constrains the native rail-domain start; `EndPoint` constrains its
end. Coordinates may be off the rail. Automatic placement retains the source
rail direction when either is present. Explicit `Parameters=` supplies stations
only for real profiles. As with ordinary sweeps, symmetric or ambiguous nearest-station
queries can require explicit parameters. A profile at the same station as a point end is an error;
the command never discards that profile. At least one real profile is required.
The viewport picker accepts these coordinate options; an interactive `Point`
getter is still pending.

Point-ended commands use a cubic rail fit for either `RefitRail` setting, as in
the captured Rhino commands. Local blending, frame options, source preservation,
current-layer output, and one-step undo follow the ordinary command. Closed
profiles retain their seam; collapsed sides become singular trims at vertices.

`Sweep1::try_new_with_point_ends` adds a `SweepEndPoints` argument. Its
`try_new_with_point_ends_and_section_preparation` variant sends only real
profiles to the preparation backend. After matching their basis and common
weight gauge, constant pole curves inherit the adjacent prepared profile's
degree, knots, and positive weights. Native builds reuse usd-brep/SMLib matching
and interpolation. Point ends count toward the existing 256-section limit.
Authored sections bypass frame round trips at their exact stations, and clamped
surface-isocurve extraction copies endpoint controls exactly so rational poles
remain collapsed.

## Evidence and limits

[Recorded validation](point-ends-validation.json) binds the sources and checks.
Ten open-profile Local command cases cover one or both ends, off-rail points,
multiple profiles, and both refit settings. Complete boundary-curve deviation
bounds certify these ruled images after accounting for U reversal. Two closed
circle cases compare complete positive rational control nets in the same tensor
basis. Portable and native SMLib command builds also compare all twelve complete
images through positive rational tensor bounds. The comparison epsilon is
`1e-7`, equal to construction tolerance.

SDK captures are retained separately: Rhino's start-only and both-ended SDK
results can reverse U and overshoot the rail, unlike the corresponding actual
commands. This implementation follows source endpoint constraints and the
qualified command cases. Three-constraint Global blending, including a real
profile between two point ends, retains the existing
[Global compatibility gap](validation.md); it is not certified by these Local
checks. General curved-rail point-ended compatibility remains unqualified.
