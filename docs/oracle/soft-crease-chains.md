# SubDCrease chain editing observations

[Soft-crease evidence](soft-crease.md) · [Chain API](../geometry/subd-chains.md)

Three immutable Rhino 8.32 captures add 48 workflows: 26 chain-option/initial-state
cases, 18 repeated-edit cases, and four anchor/no-op cases. All mouse plans finish
with acknowledged motion and clicks, and every label click enters the weight
getter. Follow-up clicks use surface stations measured from earlier native
outputs; their parent observations are recorded in the fixture assets. Existing
varying profiles are seeded from original native 3DM results.

These observations guide the upcoming profile editor. The current
[qualification](../subd-chains-qualification.json) covers control topology and
chain counts; it does not claim complete command or UI parity.

For an open `0–1–2–3` chain, representative vertex-weight sequences are:

| Initial weights | Edit | Result |
| --- | --- | --- |
| `50,50,50,50` | vertex 0 → 80 | `80,70,60,50` |
| `50,50,50,50` | vertex 1 → 80 | `50,80,65,50` |
| `0,0,0,0` | vertex 0 → 80, then vertex 1 → 20 | `80,20,10,0` |
| `0,80,40,0` loaded from 3DM | vertex 0 → 20 | `20,80,40,0` |
| `80,53⅓,26⅔,0` loaded from 3DM | vertex 0 → 20 | `20,13⅓,6⅔,0` |

Assigning an unchanged interior value does not pin that vertex against later
interpolation. Genuine profile breaks constrain later edits. A partially
weighted chain can have different incoming and outgoing values at one vertex:
with only edge `0–1` initially at 50, changing vertex 0 to 80 leaves that edge
at `[80,50]` while the next edge remains `[0,0]`.

Hard and soft weights have separate semantics. When only edge `0–1` starts hard,
editing vertex 0 to 80 makes it soft `[80,0]`. When the first two edges of a
four-edge path start hard, the same edit produces soft intervals `[80,40]` and
`[40,0]`; the remaining edges stay smooth. An entirely hard three-edge chain
instead interpolates from 80 to 100. A soft weight of 100 still differs from a
hard crease.

Closed loops retain intermediate constraints. A four-edge loop edited at vertex
0 to 80 has `80,40,0,40`; subsequently editing vertex 1 to 20 gives `80,20,0,40`.
Odd loops retain asymmetric endpoint intervals. For triangle `0–1–2–0`, the
first edit at vertex 0 to 80 produces `[80,40]`, `[0,40]`, `[40,80]` in that
orientation. A later edit at vertex 1 to 20 produces `[80,20]`, `[20,50]`,
`[50,80]`. The five-edge capture retains the corresponding middle-edge overlap;
a continuously interpolated polygon would not reproduce these recorded outputs.

CreaseThisChain and ConstantWeightChain affect only the highlighted chain,
including across multiple objects. Escape discards pending changes. Invalid
vertex weights below zero or above 100 retain the weight getter for retry.
Near-integer weights must distinguish accepted live geometry, explicit
interpolation, and native archive readback, as described in the
[soft-weight guide](../geometry/subd-soft-crease.md).
