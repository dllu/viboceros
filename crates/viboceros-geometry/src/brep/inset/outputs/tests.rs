use super::*;

#[test]
fn planar_inset_output_components_retain_old_edge_connections_and_total_area() {
    let t = Tolerance::DEFAULT;
    let frame = crate::Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        crate::Vector3::try_new(0., 0., 1.).unwrap(),
        t,
    )
    .unwrap();
    let source = Brep::try_box(frame, [[0., 8.], [0., 6.], [0., 4.]], t).unwrap();
    let before = source.clone();
    let first_edges = source.faces()[0]
        .loops()
        .iter()
        .flat_map(|l| l.trims())
        .filter_map(|t| t.edge())
        .collect::<BTreeSet<_>>();
    let adjacent = source
        .faces()
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, f)| {
            f.loops()
                .iter()
                .flat_map(|l| l.trims())
                .any(|t| t.edge().is_some_and(|e| first_edges.contains(&e)))
        })
        .unwrap()
        .0;
    let selected = [0, adjacent];
    for crease in [false, true] {
        let options = PlanarInsetOptions {
            distance: 0.3,
            crease_corners: crease,
            loose: false,
        };
        let PlanarInsetOutput::Breps(parts) = source
            .try_inset_planar_faces_output(
                &selected,
                options,
                PlanarInsetOutputMode::Separate,
                t,
                PlanarInsetOperations::default(),
            )
            .unwrap()
        else {
            panic!()
        };
        let mut counts = parts.iter().map(|p| p.faces().len()).collect::<Vec<_>>();
        counts.sort();
        assert_eq!(
            counts,
            if crease {
                vec![1, 1, 2, 10]
            } else {
                vec![1, 1, 6]
            }
        );
        assert!(parts.iter().all(|p| !p.is_solid()));
        let area = parts.iter().map(|p| p.area(t).unwrap()).sum::<Real>();
        assert!((area - source.area(t).unwrap()).abs() < 1e-8);
        let PlanarInsetOutput::Curves(curves) = source
            .try_inset_planar_faces_output(
                &selected,
                options,
                PlanarInsetOutputMode::Curves,
                t,
                PlanarInsetOperations::default(),
            )
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(curves.len(), if crease { 10 } else { 2 });
        assert_eq!(curves.iter().filter(|c| c.is_closed().unwrap()).count(), 2);
    }
    assert_eq!(source, before);
}
