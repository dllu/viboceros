#![cfg(feature = "native")]
use viboceros_geometry::{
    AffineTransform3, Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::Shape;

fn cube() -> Brep {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_box(frame, [[0., 2.]; 3], Tolerance::DEFAULT).unwrap()
}

#[test]
fn spatially_normalized_extreme_domains_stitch() {
    for exponent in [-360, 0, 360] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(scale * 1e-9, 1e-12, 1e-10).unwrap();
        let origin = Point3::try_new(0., 0., 0.).unwrap();
        let frame = Frame3::try_from_normal(
            origin,
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let input = Brep::try_box(
            frame,
            [[0., scale], [0., 2. * scale], [0., 4. * scale]],
            tolerance,
        )
        .unwrap();
        let pieces = (0..6)
            .map(|f| input.sub_brep(&[f], tolerance).unwrap())
            .collect();
        let combined = Brep::try_combine(pieces, tolerance).unwrap();
        let local = Tolerance::try_new(2.5e-10, 1e-12, 1e-10).unwrap();
        let normalized = combined
            .transformed(
                AffineTransform3::try_uniform_scale(origin, 1. / (4. * scale)).unwrap(),
                local,
            )
            .unwrap();
        let shape = Shape::from_sheet_with_normalized_domains(&normalized, local).unwrap();
        let result = shape
            .stitch(4.5e-10)
            .unwrap_or_else(|e| panic!("exponent={exponent}: {e}"));
        assert!(result.shape.to_brep(local).unwrap().is_solid());
    }
}

#[test]
fn coincident_duplicate_sheet_assembly_is_diagnosed() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let input = Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let pieces = [0, 2, 2]
        .map(|f| input.sub_brep(&[f], Tolerance::DEFAULT).unwrap())
        .to_vec();
    let combined = Brep::try_combine(pieces, Tolerance::DEFAULT).unwrap();
    let shape = Shape::from_brep(&combined, Tolerance::DEFAULT).unwrap();
    let result = shape.stitch(2.1e-9).unwrap();
    assert_eq!(result.face_origins.len(), 3);
    result.shape.to_brep(Tolerance::DEFAULT).unwrap();
}

#[test]
fn copied_sheet_stitching_closes_box_retains_faces_and_preserves_source() {
    let cube = cube();
    let faces = (0..6)
        .map(|i| cube.duplicate_faces(&[i], Tolerance::DEFAULT).unwrap())
        .collect();
    let input = Brep::try_combine(faces, Tolerance::DEFAULT).unwrap();
    let before = input.clone();
    let shape = Shape::from_brep(&input, Tolerance::DEFAULT).unwrap();
    let result = shape.stitch(1e-6).unwrap();
    let output = result.shape.to_brep(Tolerance::DEFAULT).unwrap();
    assert_eq!(result.stitched_edges, 12);
    assert_eq!(result.face_origins.len(), 6);
    assert!(output.is_solid());
    assert!((output.signed_volume(Tolerance::DEFAULT).unwrap() - 8.).abs() < 1e-9);
    assert_eq!(input, before);
    assert!(!shape.to_brep(Tolerance::DEFAULT).unwrap().is_solid());
    for distance in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(shape.stitch(distance).is_err());
    }
    let mut origins = result.face_origins;
    origins.sort_unstable();
    assert_eq!(origins, (0..6).collect::<Vec<_>>());
}

#[test]
fn partial_circular_overlap_is_split_and_stitched_by_the_kernel() {
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance,
    )
    .unwrap();
    let lower = NurbsSurface::try_cylinder(frame, 2., 0., 3.).unwrap();
    let upper = NurbsSurface::try_cylinder(frame, 2., 3., 5.).unwrap();
    let u0 = upper.parameter_at_u(0.25).unwrap();
    let u1 = upper.parameter_at_u(0.75).unwrap();
    let upper = upper.try_trimmed(u0..=u1, upper.domain_v()).unwrap();
    let input = Brep::try_combine(
        vec![
            Brep::try_surface_face(lower, tolerance).unwrap(),
            Brep::try_surface_face(upper, tolerance).unwrap(),
        ],
        tolerance,
    )
    .unwrap();
    let before = input.clone();
    let shape = Shape::from_brep(&input, tolerance).unwrap();
    let result = shape.stitch(1e-8).unwrap();
    let output = result.shape.to_brep(tolerance).unwrap();
    assert_eq!(output.faces().len(), 2);
    assert_eq!(output.edge_connected_face_components().len(), 1);
    assert!(output.is_manifold());
    assert!(!output.is_solid());
    assert!(result.stitched_edges > 0);
    assert!((output.area(tolerance).unwrap() - 16. * std::f64::consts::PI).abs() < 1e-7);
    assert_eq!(input, before);
    assert_eq!(
        shape
            .to_brep(tolerance)
            .unwrap()
            .edge_connected_face_components()
            .len(),
        2
    );
}

#[test]
fn gap_stitching_exports_measured_boundary_tolerances() {
    let tolerance = Tolerance::try_new(1e-4, 1e-12, 1e-10).unwrap();
    let gap = 1.75 * tolerance.absolute();
    let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let sheet = |y0, y1| {
        Brep::try_surface_face(
            NurbsSurface::try_bilinear([
                point(0., y0),
                point(2., y0),
                point(2., y1),
                point(0., y1),
            ])
            .unwrap(),
            tolerance,
        )
        .unwrap()
    };
    let input = Brep::try_combine(vec![sheet(-3., 0.), sheet(gap, gap + 3.)], tolerance).unwrap();
    let shape = Shape::from_brep(&input, tolerance).unwrap();
    let result = shape.stitch(1.8 * tolerance.absolute()).unwrap();
    let output = result.shape.to_brep(tolerance).unwrap();
    assert_eq!(output.edge_connected_face_components().len(), 1);
    assert!(output.is_manifold());
    assert!(output.edges().iter().any(|e| e.tolerance() >= gap));
    assert!(
        output
            .edges()
            .iter()
            .all(|e| e.tolerance() <= 2. * gap + 1e-12)
    );
    assert!(
        output
            .edges()
            .iter()
            .any(|e| e.tolerance() == tolerance.absolute())
    );
}

#[test]
fn rational_ruled_sheets_export_boundary_roundoff_with_a_certificate() {
    let tolerance = Tolerance::DEFAULT;
    let surface = |z| {
        let controls = [
            ([0., 0., 0.], 1.),
            ([1., 2., 0.], 0.5),
            ([3., 0., 0.], 1.),
            ([0., 0., z], 1.),
            ([1., 2., z], 0.5),
            ([3., 0., z], 1.),
        ]
        .into_iter()
        .map(|(p, w)| {
            WeightedPoint3::try_new(Point3::try_new(p[0], p[1], p[2]).unwrap(), w).unwrap()
        })
        .collect();
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let input = Brep::try_combine(
        vec![
            Brep::try_surface_face(surface(5.), tolerance).unwrap(),
            Brep::try_surface_face(surface(-3.), tolerance).unwrap(),
        ],
        tolerance,
    )
    .unwrap();
    let before = input.clone();
    let shape = Shape::from_brep(&input, tolerance).unwrap();
    let output = shape
        .stitch(1.8e-9)
        .unwrap()
        .shape
        .to_brep(tolerance)
        .unwrap();
    assert_eq!(output.edge_connected_face_components().len(), 1);
    assert_eq!(output.faces().len(), 2);
    assert!(output.is_manifold());
    assert!((output.area(tolerance).unwrap() - input.area(tolerance).unwrap()).abs() < 1e-8);
    assert_eq!(input, before);
}
