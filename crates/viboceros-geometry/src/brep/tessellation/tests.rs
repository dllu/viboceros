use super::*;

fn unit_box() -> Brep {
    let frame = Frame3::try_from_directions(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_box(frame, [[0.0, 1.0]; 3], Tolerance::DEFAULT).unwrap()
}

fn check_open_corner(mesh: &TriangleMesh) {
    let lines = mesh
        .filtered_edge_lines(crate::MeshEdgeFilter::Naked, Tolerance::DEFAULT)
        .unwrap();
    let length = lines
        .into_iter()
        .map(|line| line.length().unwrap())
        .sum::<Real>();
    assert!(
        (length - 6.0).abs() <= 1e-12,
        "unexpected naked boundary length {length}"
    );
    let loops = mesh
        .filtered_edge_polylines(crate::MeshEdgeFilter::Naked, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(loops.len(), 1);
    assert!(loops[0].is_closed());
    assert!(!mesh.topology().is_closed());
    assert!(mesh.topology().is_oriented());
    assert!((mesh.area().unwrap() - 2.0).abs() <= 1e-12);
}

#[test]
fn open_shell_with_unequal_face_grids_has_only_its_true_naked_boundary() {
    let mut source = unit_box()
        .duplicate_faces(&[0, 2], Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(
        (0..source.edges.len())
            .filter(|&i| source.edge_use_count(i) == Some(2))
            .count(),
        1
    );
    source.faces[0].surface = source.faces[0]
        .surface
        .try_insert_knot_u(0.23, 1)
        .unwrap()
        .try_insert_knot_v(0.71, 1)
        .unwrap();
    for mesh in [
        source.tessellate(2, Tolerance::DEFAULT).unwrap(),
        source
            .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
            .unwrap(),
    ] {
        check_open_corner(&mesh);
    }
}

#[test]
fn matching_open_shell_grids_keep_their_quadrilateral_fast_path() {
    let source = unit_box()
        .duplicate_faces(&[0, 2], Tolerance::DEFAULT)
        .unwrap();
    let mesh = source
        .polygon_mesh(0.0, true, false, Tolerance::DEFAULT)
        .unwrap();
    check_open_corner(&mesh);
    assert!(
        mesh.faces()
            .iter()
            .all(|face| matches!(face, MeshFace::Quad(_)))
    );
}

#[test]
fn naked_boundary_audit_rejects_missing_holes_and_wrong_source_faces() {
    let tolerance = Tolerance::DEFAULT;
    let p = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let outer = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
    let ring = |points: Vec<Point3>| {
        let mut closed = points;
        closed.push(closed[0]);
        crate::Polyline3::try_new(closed, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let source = Brep::try_planar_face_with_holes(
        &ring(outer.clone()),
        &[ring(vec![
            p(0.3, 0.3),
            p(0.7, 0.3),
            p(0.7, 0.7),
            p(0.3, 0.7),
        ])],
        tolerance,
    )
    .unwrap();
    let filled = TriangleMesh::try_new(outer, vec![[0, 1, 2], [0, 2, 3]], tolerance).unwrap();
    assert!(
        !source
            .mesh_boundary_conforms(&filled, &[0, 0], 4, tolerance)
            .unwrap()
    );
    let corner = unit_box().duplicate_faces(&[0, 2], tolerance).unwrap();
    let mesh = corner.tessellate(1, tolerance).unwrap();
    assert!(
        !corner
            .mesh_boundary_conforms(&mesh, &vec![0; mesh.faces().len()], 1, tolerance)
            .unwrap()
    );
    assert!(
        corner
            .mesh_boundary_conforms(&mesh, &[], 1, tolerance)
            .is_err()
    );
}

#[test]
fn independent_linear_boundary_samples_keep_quads_with_a_loose_endpoint_vertex() {
    let tolerance = Tolerance::DEFAULT;
    let mut source = unit_box().duplicate_faces(&[0], tolerance).unwrap();
    source.faces[0].surface = source.faces[0]
        .surface
        .try_insert_knot_u(0.23, 1)
        .unwrap()
        .try_insert_knot_v(0.71, 1)
        .unwrap();
    let point = source.vertices[0].point;
    source.vertices[0].point = Point3::try_new(point.x(), point.y(), point.z() + 1e-4).unwrap();
    source.vertices[0].tolerance = 1e-3;
    source.validate(tolerance).unwrap();
    let mesh = source.polygon_mesh(0.0, false, false, tolerance).unwrap();
    assert!(
        mesh.faces()
            .iter()
            .all(|face| matches!(face, MeshFace::Quad(_)))
    );
}

#[test]
fn jagged_trimmed_face_meshes_do_not_snap_to_shared_edge_geometry() {
    use crate::{Circle3, PointMorph};
    struct Bend;
    impl PointMorph for Bend {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x(), p.y(), p.z() + p.x().powi(2))
        }
    }
    let circle = |r| {
        Circle3::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            r,
            Vector3::try_new(0.0, 0.0, 1.0)
                .unwrap()
                .normalized(Tolerance::DEFAULT)
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let planar =
        Brep::try_planar_face_with_holes(&circle(1.0), &[circle(0.3)], Tolerance::DEFAULT).unwrap();
    let curved = Bend
        .morph_brep(&planar, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap())
        .unwrap();
    for mut source in [planar, curved] {
        let before = source
            .polygon_mesh(0.0, false, true, Tolerance::DEFAULT)
            .unwrap();
        let offset = AffineTransform3::from_translation(Vector3::try_new(0.0, 0.0, 1e-4).unwrap());
        for vertex in &mut source.vertices {
            vertex.point = offset.transform_point(vertex.point).unwrap();
            vertex.tolerance = 1e-3;
        }
        for edge in &mut source.edges {
            edge.curve = edge.curve.transformed(offset).unwrap();
            edge.tolerance = 1e-3;
        }
        source.validate(Tolerance::DEFAULT).unwrap();
        let after = source
            .polygon_mesh(0.0, false, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(
            after, before,
            "jagged meshing must depend on the face, not the edge fit"
        );
    }
}

#[test]
fn conforming_fallback_does_not_bridge_an_interior_surface_jump() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_directions(
        p(0.0, 0.0, 0.0),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let mut source = Brep::try_box(frame, [[0.0, 1.0]; 3], tolerance).unwrap();
    let original = &source.faces[0].surface;
    let mut controls = Vec::new();
    for (j, v) in [0.0, 0.5, 1.0].into_iter().enumerate() {
        for (i, u) in [0.0, 0.5, 0.5, 1.0].into_iter().enumerate() {
            let point = original.evaluate(u, v).unwrap();
            let z = point.z() + if i == 1 && j == 1 { 0.1 } else { 0.0 };
            controls.push(WeightedPoint3::try_new(p(point.x(), point.y(), z), 1.0).unwrap());
        }
    }
    source.faces[0].surface = NurbsSurface::try_new_rational(
        1,
        2,
        4,
        3,
        controls,
        vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    // All boundary curves still agree: topology/boundary validation alone
    // does not certify the face's interior continuity.
    source.validate(tolerance).unwrap();
    assert!(source.tessellate(2, tolerance).is_err());
    let open = source.duplicate_faces(&[0], tolerance).unwrap();
    assert!(open.tessellate(2, tolerance).is_err());
}

#[test]
fn unequal_face_knots_and_independent_edge_speed_mesh_without_t_junctions() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_directions(
        p(0.0, 0.0, 0.0),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let mut source = Brep::try_box(frame, [[0.0, 1.0]; 3], tolerance).unwrap();
    source.faces[0].surface = source.faces[0].surface.try_insert_knot_u(0.23, 1).unwrap();
    source.faces[1].surface = source.faces[1].surface.try_insert_knot_v(0.71, 1).unwrap();
    let edge = &mut source.edges[0];
    let a = edge.curve.evaluate(*edge.curve.domain().start()).unwrap();
    let b = edge.curve.evaluate(*edge.curve.domain().end()).unwrap();
    let middle = Point3::try_from(std::array::from_fn(|i| {
        a.to_array()[i] * 0.75 + b.to_array()[i] * 0.25
    }))
    .unwrap();
    edge.curve = NurbsCurve::try_clamped_uniform(2, vec![a, middle, b]).unwrap();
    source.validate(tolerance).unwrap();
    for mesh in [
        source.tessellate(2, tolerance).unwrap(),
        source.polygon_mesh(0.0, false, false, tolerance).unwrap(),
    ] {
        assert!(mesh.topology().is_solid());
        assert_eq!(mesh.topology().orientation_conflict_edge_count(), 0);
    }
}
