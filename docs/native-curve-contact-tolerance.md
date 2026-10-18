# Native curve contact tolerance

Native curve intersection now retains bounded contacts within the caller's
distance tolerance when both strict Newton attempts stop short of arithmetic
coincidence. This fixes an `Intersect` discrepancy found while investigating
curved surface fillets.

For example, consider `A(t) = (2t, 4t(1-t), d)` and
`B(s) = (2s, 0.5, 0)`. At the two analytic XY crossings,
`t = s = (1 ± sqrt(0.5))/2`, their separation is `abs(d)`.
With `d = 5e-9` and tolerance `1e-8`, Rhino returns two contacts while the
preceding native producer returned none. Both engines return none for
`abs(d) = 1.5e-8`.

The existing native solver first seeks tight physical coincidence, then retries
with its separate tangent objective. Those targets and iteration limits remain
in place. After both attempts fail, the repair evaluates their best parameter
pairs on the original native curves. Parameters must be finite and inside both
requested intervals; the measured spatial gap must be strictly below the
existing contact tolerance. The better qualifying witness enters the existing
refinement and filtering path. No tolerance is increased, and the strict
near-tangent root search and source-space acceptance remain active.

The direct C ABI probe checks 252 cases: polynomial parabolas and rational
quarter circles, seven signed gap factors, three physical scales, three axis
placements and both argument orders. It checks independent analytic locations,
paired spatial gaps, finite interval membership and source purity. Parameters
are copied after both native input curves are destroyed, then read after their
native event buffer is destroyed. The preceding producer passes 192 cases and
misses 60 polynomial contact cases. The repair passes all 252 normally and under
AddressSanitizer. Maximum observed position error is about `1.01e-14 * scale`;
maximum accepted gap is about `5.01e-9 * scale` against `1e-8 * scale`.
ASan instruments SMLib, SM_API, the bridge and the direct probe; leak detection
is disabled.

A public Rust regression adds 378 recipes across world scales `1e-40` to
`1e40`, translated and permuted coordinates, distinct distant/tiny parameter
charts and common signed weight gauges. Its absolute budget dominates the
shared relative scale floor. It checks paired parameters, original-curve gaps,
analytic point locations, rejection outside the budget and unchanged sources.
The existing shallow tangency, two-root, overlap and unsupported-basis cases
also remain covered.

Five fresh Rhino `Intersect` command recipes retain complete protocol values
for coplanar controls, positive/negative in-tolerance gaps and an out-of-tolerance
gap. Their output coordinates, attributes and input selection compare within
`1e-8`, with no comparison-time normalization. Native output points retain their
paired-point midpoint; Rhino reports points on the second curve in these
captures. Both coordinates and this small difference remain in the raw records.

See the [validation record](native-curve-contact-tolerance-validation.json) for
the complete normal/baseline/ASan data, saved command replays, shared bridge and
command checks, and source hashes. This repair does not complete the curved
extended [surface fillet](native-surface-fillets.md); support continuation and
end-cap construction still require work. General intersection completeness and
performance remain unqualified. Upstream `repo.sh`/`prog_test`, USD, Python-binding,
OCCT and GUI harnesses were not run; this ARM bridge builds the actual SMLib
and SM_API libraries.

```sh
cargo test --release -p viboceros-smlib --features native --test curve_intersections
cargo test --release -p viboceros-oracle --features native-smlib --test curve_contacts_tolerance
```
