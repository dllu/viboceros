//! Write shared, unedited 3DM operands for licensed Dir comparisons.
use std::path::PathBuf;
use viboceros_geometry::{
    Brep, Frame3, NurbsSurface, Point3, Polyline3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, write_3dm_file};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/brep_direction");
    std::fs::create_dir_all(&root)?;
    let t = Tolerance::DEFAULT;
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let frame = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.)?,
        Vector3::try_new(0., 1., 0.)?,
        t,
    )?;
    let polygon = |points| Polyline3::try_new(points, t).unwrap().to_nurbs().unwrap();
    let outer = polygon(vec![
        p(0., 0., 0.),
        p(8., 0., 0.),
        p(8., 6., 0.),
        p(0., 6., 0.),
        p(0., 0., 0.),
    ]);
    let hole = polygon(vec![
        p(2., 2., 0.),
        p(2., 4., 0.),
        p(5., 4., 0.),
        p(5., 2., 0.),
        p(2., 2., 0.),
    ]);
    let holed = Brep::try_planar_face_with_holes(&outer, &[hole], t)?;
    let controls = [
        [0., 0., 0.],
        [2., 3., 1.],
        [5., 0., 2.],
        [0., 4., 1.],
        [2., 7., 3.],
        [5., 4., 4.],
    ]
    .into_iter()
    .enumerate()
    .map(|(i, p)| WeightedPoint3::try_new(Point3::try_from(p)?, if i % 3 == 1 { 0.5 } else { 1. }))
    .collect::<Result<Vec<_>, _>>()?;
    let warped = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        controls,
        vec![-4., -4., -4., 2., 2., 2.],
        vec![3., 3., 9., 9.],
    )?;
    let mut sources = vec![
        ("holed", holed.clone()),
        ("holed_reversed", holed.reversed()),
        ("warped", Brep::try_surface_face(warped, t)?),
        (
            "cylinder",
            Brep::try_surface_face(NurbsSurface::try_cylinder(frame, 2., -3., 5.)?, t)?,
        ),
        (
            "sphere",
            Brep::try_surface_face(NurbsSurface::try_sphere(frame, 2.)?, t)?,
        ),
        (
            "cone",
            Brep::try_surface_face(NurbsSurface::try_cone(frame, 2., 5.)?, t)?,
        ),
        (
            "torus",
            Brep::try_surface_face(NurbsSurface::try_torus(frame, 4., 1.)?, t)?,
        ),
        (
            "box",
            Brep::try_box(frame, [[-2., 3.], [-1., 4.], [0., 5.]], t)?,
        ),
    ];
    for (name, brep) in sources.drain(..) {
        let path = root.join(format!("{name}.3dm"));
        if path.exists() {
            return Err(format!("shared operands are immutable: {}", path.display()).into());
        }
        let model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Geometry".into(),
                color: [30, 60, 90],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![ThreeDmObject::new(ThreeDmGeometry::Brep(brep), 0)],
        );
        write_3dm_file(&path, &model)?;
        println!("{}", path.display());
    }
    Ok(())
}
