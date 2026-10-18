use super::*;

fn sheet(x: Real) -> Brep {
    let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            point(x, 0.),
            point(x + 1., 0.),
            point(x + 1., 1.),
            point(x, 1.),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

/// Retain the original exhaustive native participation decision as an
/// independent reference. Each call runs the actual kernel, not a mock.
fn exhaustive_pick(sources: &[&Brep]) -> (Vec<BrepJoinComponent>, bool, usize) {
    let mut assemblies = 1;
    let result = assemble(sources, 2.1e-9, Tolerance::DEFAULT).unwrap();
    let latest = sources.len() - 1;
    if sources.len() == 2 {
        let participates = result
            .candidate_source_pairs
            .iter()
            .any(|p| p.contains(&latest));
        return (result.components, participates, assemblies);
    }
    let mut participates = false;
    for a in 0..sources.len() {
        for b in a + 1..sources.len() {
            assemblies += 1;
            let pair = assemble(&[sources[a], sources[b]], 2.1e-9, Tolerance::DEFAULT).unwrap();
            if b == latest && pair.components.iter().any(|c| c.joined_edge_count > 0) {
                participates = true;
            }
        }
    }
    // The command also requires a surviving join in the batch assembly.
    participates &= result.components.iter().any(|c| c.joined_edge_count > 0);
    (result.components, participates, assemblies)
}

fn compare_pick(sources: &[Brep], expected: bool, expected_assemblies: Option<usize>) {
    let before = sources.to_vec();
    let borrowed = sources.iter().collect::<Vec<_>>();
    let (reference, reference_participates, exhaustive_assemblies) = exhaustive_pick(&borrowed);
    let mut assemblies = 0;
    let (components, participates) = assemble_pick_with(&borrowed, |sources| {
        assemblies += 1;
        assemble(sources, 2.1e-9, Tolerance::DEFAULT)
    })
    .unwrap();
    assert_eq!(components, reference);
    assert_eq!(participates, reference_participates);
    assert_eq!(participates, expected);
    assert!(assemblies <= sources.len());
    if let Some(expected) = expected_assemblies {
        assert_eq!(assemblies, expected);
    }
    assert_eq!(sources, before);
    eprintln!(
        "PICK_ASSEMBLIES sources={} participates={participates} exhaustive={exhaustive_assemblies} latest={assemblies}",
        sources.len()
    );
}

#[test]
fn picked_sheet_participation_matches_exhaustive_stitching_with_bounded_work() {
    compare_pick(&[sheet(0.), sheet(1.)], true, Some(1));
    compare_pick(&[sheet(0.), sheet(100.), sheet(200.)], false, Some(1));
    compare_pick(&[sheet(0.), sheet(1.), sheet(100.)], false, Some(3));
    compare_pick(&[sheet(0.), sheet(1.), sheet(-1.)], true, Some(2));
    let chain = (0..12).map(|x| sheet(x as Real)).collect::<Vec<_>>();
    compare_pick(&chain, true, Some(12));
    let mut early_contact = chain;
    early_contact.push(sheet(-1.));
    compare_pick(&early_contact, true, Some(2));
    *early_contact.last_mut().unwrap() = sheet(100.);
    compare_pick(&early_contact, false, Some(13));
}

#[test]
fn competing_pick_participation_retains_original_contacts_and_source_order() {
    let cube = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 5.]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let floor = cube.sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let wall = cube.sub_brep(&[2], Tolerance::DEFAULT).unwrap();
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        for reverse in [false, true] {
            let pieces = [
                floor.clone(),
                wall.clone(),
                if reverse {
                    wall.reversed()
                } else {
                    wall.clone()
                },
            ];
            let sources = order.map(|i| pieces[i].clone());
            compare_pick(&sources, true, None);
        }
    }
}

#[test]
fn unknown_curved_component_orientation_retains_the_native_ray_path() {
    let t = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let surface =
        NurbsSurface::try_sphere(CommandContext::default().construction_plane, 2.).unwrap();
    let u = [0.25, 0.5, 0.75].map(|x| surface.parameter_at_u(x).unwrap());
    let v = [surface.parameter_at_v(0.5).unwrap()];
    let sphere = Brep::try_surface_grid(&surface, &u, &v, Tolerance::DEFAULT).unwrap();
    let rotation = AffineTransform3::try_rotation(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 2., 3.).unwrap().normalized(t).unwrap(),
        0.7,
    )
    .unwrap();
    let sphere = sphere.transformed(rotation, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        sphere.solid_orientation().unwrap(),
        BrepSolidOrientation::Unknown
    );
    for reversed in [false, true] {
        let sources = (0..sphere.faces().len())
            .map(|i| {
                let b = sphere.sub_brep(&[i], Tolerance::DEFAULT).unwrap();
                if reversed { b.reversed() } else { b }
            })
            .collect::<Vec<_>>();
        let before = sources.clone();
        let combined = Brep::try_combine(sources.clone(), t).unwrap();
        let shape = Shape::from_brep(&combined, t).unwrap();
        let stitched = shape.stitch(0.0018).unwrap();
        assert!(stitched.stitched_edges > 0);
        let topology = stitched.to_brep_preserving_source_uv(&combined, t).unwrap();
        assert!(topology.is_solid());
        assert_eq!(
            topology.solid_orientation().unwrap(),
            BrepSolidOrientation::Unknown
        );
        let refs = sources.iter().collect::<Vec<_>>();
        let legacy = assemble_with_orientation(&refs, 0.0018, t, true);
        let actual = assemble(&refs, 0.0018, t);
        match (actual, legacy) {
            (Ok(actual), Ok(expected)) => {
                eprintln!(
                    "UNKNOWN_CURVED reversed={reversed} native_ray=success components={}",
                    expected.components.len()
                );
                assert_eq!(actual, expected);
            }
            (Err(actual), Err(expected)) => {
                eprintln!("UNKNOWN_CURVED reversed={reversed} native_ray=error {expected}");
                assert_eq!(actual.to_string(), expected.to_string());
            }
            (actual, expected) => panic!("reversed={reversed}: {actual:?} != {expected:?}"),
        }
        assert_eq!(sources, before);
    }
}
