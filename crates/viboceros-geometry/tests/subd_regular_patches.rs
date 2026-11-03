use viboceros_geometry::SubD;
#[path = "support/positive_tensor.rs"]
mod bound;
#[path = "support/subd_to_nurbs.rs"]
mod native;
#[path = "support/nurbs_surface.rs"]
mod surface;

#[test]
fn regular_torus_matches_every_native_bezier_control_over_the_complete_image() {
    let value = native::case("torus");
    let source = native::source(&value["before"]);
    let archive = source.archive().to_vec();
    let patches = source.regular_nurbs_patches().unwrap();
    let faces = value["output"]["geometry"]["faces"].as_array().unwrap();
    assert_eq!(patches.len(), 32);
    assert_eq!(faces.len(), patches.len());
    let mut maximum = 0_f64;
    for (patch, face) in patches.iter().zip(faces) {
        let expected = surface::from_definition(&face["definition"]);
        // Matching nonnegative Bernstein bases bound the entire patch by the
        // largest control displacement, rather than only a station sample.
        maximum = maximum.max(bound::complete_bound(patch.as_ref().unwrap(), &expected));
    }
    assert!(maximum < 1e-12, "full-image bound {maximum}");
    assert_eq!(source.archive(), archive);
}

fn check_regular_stations(source: &SubD) -> usize {
    let patches = source.regular_nurbs_patches().unwrap();
    assert_eq!(patches.len(), source.faces().len());
    let grids = source.limit_face_grids(3).unwrap();
    let mut count = 0;
    for grid in grids {
        let Some(patch) = &patches[grid.face] else {
            continue;
        };
        assert!(grid.corner.is_none());
        count += 1;
        for v in 0..=grid.side {
            for u in 0..=grid.side {
                let p = patch
                    .evaluate(u as f64 / grid.side as f64, v as f64 / grid.side as f64)
                    .unwrap();
                assert!(p.distance_to(grid.points[v * (grid.side + 1) + u]).unwrap() < 1e-11);
            }
        }
    }
    count
}

#[test]
fn mixed_regular_and_extraordinary_faces_retain_their_source_ordinals() {
    let source = native::source(&native::case("grid")["before"]);
    let count = check_regular_stations(&source);
    // The public extractor checks the full surrounding control stencil. This
    // small grid has no patch isolated far enough from a tagged corner.
    assert_eq!(count, 0);
    let smooth = native::source(&native::case("box")["before"]);
    assert_eq!(check_regular_stations(&smooth), 0);
    let refined = smooth.subdivided(3).unwrap();
    let count = check_regular_stations(&refined);
    assert!(count > 0 && count < refined.faces().len());
    assert_eq!(
        check_regular_stations(&smooth.subdivided_faces(&[0]).unwrap()),
        0
    );
    let torus = native::source(&native::case("torus")["before"])
        .subdivided(1)
        .unwrap();
    assert_eq!(check_regular_stations(&torus), torus.faces().len());
    let ngon = native::source(&native::case("pentagon")["before"]);
    assert_eq!(check_regular_stations(&ngon), 0);
}

#[test]
fn soft_sharpness_is_never_misreported_as_an_exact_regular_patch() {
    let source = native::source(&native::case("torus")["before"]);
    let (soft, _) = source
        .try_with_soft_crease_weights(&[(0, [25., 75.])])
        .unwrap();
    let count = check_regular_stations(&soft);
    assert!(count > 0 && count < source.faces().len());
    assert!(viboceros_opennurbs::subd_regular_patches(b"invalid archive").is_err());
}
