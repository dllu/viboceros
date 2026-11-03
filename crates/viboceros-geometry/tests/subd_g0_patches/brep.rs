use super::*;
use viboceros_geometry::{Point3, SubD, Tolerance};

pub(super) fn check(source: &SubD, value: &Value) {
    crate::compare::check(&source.to_nurbs_brep_g0(Tolerance::DEFAULT).unwrap(), value);
}

#[test]
fn coincident_shells_keep_independent_topology() {
    let source = native::source(&native::case("box")["before"]);
    let mut points = source.control_points().to_vec();
    points.extend_from_within(..);
    let mut faces = source.faces().to_vec();
    faces.extend(
        source
            .faces()
            .iter()
            .map(|f| f.iter().map(|i| i + 8).collect()),
    );
    let doubled = SubD::try_from_polygons(points, faces).unwrap();
    let brep = doubled.to_nurbs_brep_g0(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (16, 24, 12)
    );
    assert!(brep.is_solid());
    // Source identities distinguish two coincident shells throughout assembly.
    assert_eq!(brep.edge_connected_face_components().len(), 2);
}

#[test]
fn ngon_midpoints_split_adjacent_quad_trims_without_splitting_their_surfaces() {
    let points = (0..2)
        .flat_map(|z| {
            (0..5).map(move |i| {
                let a = std::f64::consts::TAU * i as f64 / 5.;
                Point3::try_new(3. * a.cos(), 3. * a.sin(), 2. * z as f64).unwrap()
            })
        })
        .collect();
    let mut faces = (0..5)
        .map(|i| vec![i, (i + 1) % 5, (i + 1) % 5 + 5, i + 5])
        .collect::<Vec<_>>();
    faces.push(vec![4, 3, 2, 1, 0]);
    faces.push(vec![5, 6, 7, 8, 9]);
    let source = SubD::try_from_polygons(points, faces).unwrap();
    let archive = source.archive().to_vec();
    let brep = source.to_nurbs_brep_g0(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (22, 35, 15)
    );
    assert!(brep.is_solid());
    for face in &brep.faces()[..5] {
        assert_eq!(face.loops()[0].trims().len(), 6);
    }
    for face in &brep.faces()[5..] {
        assert_eq!(face.loops()[0].trims().len(), 4);
    }
    assert_eq!(source.archive(), archive);
}
