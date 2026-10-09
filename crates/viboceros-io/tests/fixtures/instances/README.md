# OpenNURBS block fixtures

`generate.cpp` is an original fixture producer using the vendored public OpenNURBS
API. It writes `nested_blocks.3dm` and `hidden_blocks.3dm`, validating their model
components and geometry. `missing_block.3dm` and `cyclic_block.3dm` intentionally
contain invalid reference graphs. No proprietary implementation or native Rhino
capture is involved.

The prototype has a point, line, colored/normal-bearing point cloud and unit box.
The nested placement scales by `(2,3,4)`, translates by `(1,2,3)`, then rotates
90 degrees about Z and translates by `(10,20,30)`. The point `(1,2,3)` becomes
`(2,23,45)`, and the box has volume 24. The second occurrence translates the
prototype by `(-10,-20,-30)`. An unrelated point remains at `(99,98,97)`.

`audit.cpp` reads the nested fixture to create a reflected placement and
intentionally invalid projective/singular placements. It also creates a 20-level
acyclic branching graph of empty definitions. This small archive would require
exponential traversal without a visit budget, even though it has no leaf geometry.
Run its executable with the nested fixture path and output directory as arguments.

Regenerate with a C++17 compiler linked to `opennurbsStatic` and its vendored
zlib, freetype and UUID archives from a completed `viboceros-io` build. Run the
resulting executable with this directory as its sole argument. Archive metadata
can change when regenerating; inspect geometric values and update the
[provenance](../../../../../docs/three-dm-block-provenance.json) after validation.
