# Command-first Join participation

Native Join and JoinCopy previously repeated every two-source stitching
comparison after each pick. The command only uses these comparisons to decide
whether the newest pick contacts an original accepted source. With `n > 2`
attempted sources, the previous path performed one batch assembly plus
`n(n-1)/2` two-source assemblies.

The native adapter now returns the batch components and the newest pick's
participation separately. It compares that pick with the original accepted
sources in pick order and stops at the first confirmed contact. It performs
at most `n-1` pair probes, in addition to the batch. Two-source attempts reuse
their batch result. A batch with no surviving cross-source join skips the
pair probes because the command cannot accept that pick.

Participation still comes from two-source native stitching of original
operands. A batch's resulting edge ownership is insufficient to establish
that contact: other operands can affect splitting and inherited tolerances.
The batch's topology does not depend on these participation probes. A
contacting pick may remain a separate output after batch ambiguity resolution,
as before. Preselected assembly and the default Rust boundary matcher keep
their existing behavior.

Two native tests run the previous exhaustive procedure and the new path on
actual kernel inputs, comparing every returned B-rep component, source index,
joined-edge count and participation decision. They check immutable inputs and
count assemblies, including all six orders of a three-way competing boundary
with both wall senses. The observed counts include:

| Attempt | Previous assemblies | Current assemblies |
| --- | ---: | ---: |
| Two adjacent sheets | 1 | 1 |
| Three unrelated sheets | 4 | 1 |
| 12-sheet chain, contact at the last prior source | 67 | 12 |
| 13th sheet contacts the first prior source | 79 | 2 |
| Unrelated 13th sheet alongside an existing chain | 79 | 13 |

The [validation record](join-pick-validation.json) retains 336 native
before/after cases from five existing Join archives. Their complete audit
responses are byte-for-byte identical, including geometry, raw table order,
tolerances, metadata, source retention and selection. The gzip archives retain
those exact response bytes once for both versions; the validation record
includes the uncompressed hashes, fixture hashes and source revisions.
Existing strict Rhino differences remain; this is a reduction in native work,
not a new parity claim. Audit timing fields are placeholders, and these
operation counts do not establish an end-to-end speedup or Rhino performance
parity. The growing batch is still rebuilt for each attempted pick.

Run the operation-count tests and check one retained replay:

```sh
cargo test -p viboceros-command --features native-smlib join::breps::native::tests -- --nocapture
cargo run -p viboceros-oracle --features native-smlib -- --audit \
  tools/rhino_oracle/fixtures/join_surfaces.json /tmp/join-pick-surfaces.json
gzip -dc docs/join-pick-surfaces-reference.json.gz | cmp - /tmp/join-pick-surfaces.json
```

The [closed-Join orientation](native-closed-join.md) policy can repeat a logical
source assembly once when an exact orientation witness is unknown, using the
previous native ray path. The open-sheet operation counts above are unchanged;
the bound on logical source assemblies does not include that closed-orientation
retry.
