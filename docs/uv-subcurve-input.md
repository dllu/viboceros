# Temporary subcurves in UV commands

[ApplyCrv](commands/apply-curves.md) · [CreateUVCrv](commands/create-uv-curves.md)

At either command's curve/point selection prompt, type `SubCrv`, select one
curve, then pick or type its start and directed end. Selection returns to the
parent command; repeat to use several ranges from the same source. `SelNone`
discards accepted temporary ranges as well as whole-object picks. Escape cancels
the parent command and restores its initial whole-object selection.

Scripts can supply native parameter intervals, without selecting their sources:

```text
ApplyCrv Surface=surface-uuid SubCrv=curve-uuid,start,end
CreateUVCrv Surface=surface-uuid [Face=index] SubCrv=curve-uuid,start,end
```

Repeat the option for multiple ranges. Other selected curves and points remain
whole inputs. Decreasing open intervals reverse direction; decreasing closed
intervals cross the existing seam. No temporary document objects or history
entries are created while picking. Invalid, missing or restricted references
and invalid intervals fail atomically.

Native temporary inputs have default attributes: their mapped outputs use the
current layer, have no name or source user text, and receive no group memberships.
Whole-object inputs keep the existing attribute/group policies. Original curves
and their groups remain unchanged. The parent command owns the output Undo entry.

The [14 native recipes](../tools/rhino_oracle/fixtures/uv_subcurve_input_command.json)
ran on private Xvfb with scheme `VibocerosOracleUVSubcurveClear20261007`.
[Raw records](../tools/rhino_oracle/observations/uv_subcurve_input_command.json)
contain complete source definitions, 24 output curves with 792 stations, two
output points, getter history, attributes, layers, groups, selection and Undo/Redo.
Rust command tests compare every captured locus and directed endpoints at `1e-6`.
Application tests exercise typed/mouse input, repeated ranges, invalid endpoints,
selection clearing, interruption and cancellation. See [provenance](uv-subcurve-input-provenance.json).

The [Python fixtures](../tools/rhino_oracle/fixtures/uv_subcurve_input_local.json)
retain original full sources and requested parameter ranges, with `selected=false`
for cleared inputs. The `apply_uv_curves` / `create_uv_curves` operations accept
`subcurves: [[start,end], ...]` on curve definitions. A nonempty range list uses
temporary inputs; otherwise `selected` defaults to true. Optional `groups` lists
index the surface at zero and subsequent input objects from one.

One closed CreateUVCrv result has identical ordered linear controls but different
knot spacing: paired normalized stations differ by up to `0.50625`. This is a
retained parameter-speed discrepancy; all captured loci agree within `1e-6`.
[Inline length input](subcurve-length-confirmation.md) now accepts a number followed
by a direction confirmation. Direction locking and B-rep edge references remain
unsupported. Existing native UV-sizing and off-surface projection limits still apply.
[Signed-length script inputs](signed-length-subcurves.md) now provide
`SubCrvLength=curve,anchor,signed-length` independently of the inline getter.

Public reference: [McNeel inline SubCrv input](https://docs.mcneel.com/rhino/8/help/en-us/commands/subcrv.htm).

```sh
cargo test -p viboceros-command --release uv_inputs
cargo test --release --bin viboceros app::tests::uv_subcurve_input
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/uv_subcurve_input_local.json --timeout 600 --output docs/uv-subcurve-input-local.json
```

[Direction locking](subcurve-direction.md) shares the standalone hover capture
and immediately accepts numeric lengths on the captured side.
