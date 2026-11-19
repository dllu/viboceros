use super::*;

#[test]
fn oblique_boundary_preserves_the_rim_vertices_at_cutter_contacts() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (warped, rectangle, contacts) in [
        (
            false,
            [-2., -2.2876659672906228, 3.7, 2.7],
            vec![point(
                0.8502426312860052,
                -2.2876659606568275,
                4.999999999055017,
            )],
        ),
        (
            true,
            [-2., -2.7, 3.136774681710049, 2.7],
            vec![point(
                3.1367743817100497,
                0.0028735339294426853,
                5.357915246130641,
            )],
        ),
        (
            false,
            [-2., -2.287665667290623, 3.7, 2.7],
            vec![
                point(0.8513796272865428, -2.2876656672906233, 5.),
                point(0.849100484706434, -2.287665667290623, 5.),
            ],
        ),
        (
            true,
            [-2., -2.7, 3.13677408171005, 2.7],
            vec![
                point(3.13677408171005, 0.004033557582568901, 5.3579733504657865),
                point(3.136774081710049, 0.0017133801840117247, 5.357857101186666),
            ],
        ),
    ] {
        let support = cutter(rectangle, warped);
        let (mut doc, source, reference) = scene(tolerance, &support, false);
        let commands = CommandRegistry::with_builtins();
        let options = [
            "Direction=1,0,6",
            "DraftAngle=8",
            "Solid=Yes",
            "DeleteInput=Yes",
        ];
        let result = pick(&mut doc, &commands, reference, &support, &options);
        assert_eq!(
            result.unwrap_or_else(|error| panic!("{rectangle:?}: {error}")),
            TaperedBoundaryOutcome::Cut(1),
            "{rectangle:?}"
        );
        let Geometry::Brep(brep) = doc
            .objects()
            .find(|o| o.id() != reference.object)
            .unwrap()
            .geometry()
        else {
            panic!("expected a capped sweep")
        };
        assert!(brep.is_solid());
        assert_eq!(brep.edges().len(), 12 + contacts.len(), "{rectangle:?}");
        // The analytic circle sweep and Rhino's fitted sweep differ by up
        // to 3e-5 along these nearly tangent crossings. Normal distance to
        // the cutter still has to meet the model tolerance.
        let position_epsilon = if contacts.len() == 1 {
            2. * tolerance.absolute()
        } else {
            4e-5
        };
        for expected in contacts {
            let vertex = brep
                .vertices()
                .iter()
                .map(|vertex| vertex.point())
                .min_by(|a, b| {
                    a.distance_to(expected)
                        .unwrap()
                        .total_cmp(&b.distance_to(expected).unwrap())
                })
                .unwrap();
            assert!(
                vertex.distance_to(expected).unwrap() < position_epsilon,
                "{vertex:?} vs {expected:?}"
            );
            let (u, v) = support.closest_parameters(vertex, tolerance).unwrap();
            let edge_point = if warped {
                support.evaluate(1., v)
            } else {
                support.evaluate(u, 0.)
            };
            assert!(edge_point.unwrap().is_near(vertex, tolerance));
        }
        assert!(doc.object(source).is_none());
    }
}
