# Rational line arc length

A degree-one NURBS span follows a straight segment, but unequal weights change
its parameter speed. Sampling that derivative can miss almost all of its motion:
the previous length implementation reported almost zero for a segment of length
2 with weight ratio 2^100.

Degree-one spans now use exact Bezier extraction, endpoint distances and finite
length accumulation. Equal-sign endpoint weights guarantee a finite, monotone
projective traversal. Opposite signs imply a denominator pole and fail before
length or station output. Higher-degree integration is unchanged by this path.

For endpoint weights w0 and w1, native unit parameter u maps to geometric
fraction w1*u/(w0*(1-u)+w1*u). A local distance d on a segment of length L maps
back through d*w0/((L-d)*w1+d*w0). Forward and inverse expressions use exact
rational arithmetic before final binary64 rounding. Sampling straight geometry
from the distance fraction preserves point locations when weighted native
parameters round near an endpoint. Binary64 parameters still cannot represent
every station in extreme or shifted domains; Split can reject collapsed cuts.
Before trimming, Divide verifies that evaluating each native cut parameter
agrees with its geometric station at the document tolerance. Unrepresentable
cuts return a dedicated error before changing geometry or history, including
polycurve offsets where a rounded parameter lands at a different endpoint.

The arc-length sampler carries these spans through polycurve parameter maps.
They need neither speed integration nor lookup tables. Regressions cover
extreme and reversed weights, equal-sign negative weights, multispan corners,
trimmed domains, poles, forward/inverse queries and polycurve point/tangent
sampling. This does not qualify every signed rational curve or parameter domain.

Direct degree-one sampling retains the original span domains instead of applying
integration normalization. A subnormal span beside very large domains can still
be measured and sampled from its straight geometry, even when normalization
would erase the interval. The normalization routine continues to reject that
loss for consumers that need it.

[Native recipes](../tools/rhino_oracle/fixtures/divide_rational_line.json) cover
32 count/length point and Split cases. Licensed Rhino runs use private Xvfb.
Additional native-only inspection records command history and completion state:
the captured extreme-weight zero-output cases end normally and are not pending
getters. Those records remain diagnostics; mathematical line geometry is not
replaced with zero output to match them. The probe now requires idle execution
and cancels only its own unfinished macro before source cleanup.

Sixteen moderate-weight recipes match at 5e-7 absolute plus 1e-12 relative
epsilon. The sixteen extreme-weight records remain full-field diagnostics.
Local Split invocations reject collapsed or unrepresentable parameter cuts;
point outputs are covered separately by the geometric regressions. No case matches
every field at the tighter 1e-9 absolute epsilon in this capture.

[Comparison](divide-rational-line-comparison.json) and
[provenance](rational-line-provenance.json) retain final results and scope.
Extreme-weight native compatibility, unrepresentable parameters, broader
rational integration and performance qualification remain open.
