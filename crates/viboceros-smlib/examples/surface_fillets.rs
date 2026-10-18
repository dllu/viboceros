//! Retain native fillet geometry independently of fallible mass-property queries.
use serde_json::json;
use std::{fs, path::PathBuf};
use viboceros_geometry::{NurbsSurface, Point3, Tolerance, WeightedPoint3};
use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, write_3dm_file};
use viboceros_smlib::{SurfaceFilletOptions, fillet_surfaces};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/viboceros-surface-fillets".into()),
    );
    fs::create_dir_all(&directory)?;
    let plane = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        [
            [-10., -10., 0.],
            [10., -10., 0.],
            [-10., 10., 0.],
            [10., 10., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )?;
    let cylinder = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        [-5., 5.]
            .into_iter()
            .flat_map(|z| {
                (0..3).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            if i < 2 { 5. } else { 0. },
                            if i > 0 { 5. } else { 0. },
                            z,
                        )
                        .unwrap(),
                        if i == 1 { 0.5_f64.sqrt() } else { 1. },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )?;
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8)?;
    for extend in [false, true] {
        let brep = fillet_surfaces(
            &plane,
            &cylinder,
            SurfaceFilletOptions {
                radius: 1.,
                extend,
                ..SurfaceFilletOptions::default()
            },
            tolerance,
        )?
        .ok_or("No surface fillet")?;
        let model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Fillet".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![ThreeDmObject::new(ThreeDmGeometry::Brep(brep.clone()), 0)],
        );
        let name = format!("cylinder-{extend}");
        // Persist the exact body before any query that may fail.
        write_3dm_file(directory.join(format!("{name}.3dm")), &model)?;
        let faces =
            brep.faces()
                .iter()
                .map(|f| {
                    let s = f.surface();
                    let trims = f.loops().iter().flat_map(|l| l.trims()).map(|t| json!({
                "edge":t.edge(), "iso":format!("{:?}",t.iso()),
                "degree":t.curve().degree(), "knots":t.curve().knots(),
                "control_points":t.curve().control_points().iter().map(|p|
                    json!([p.point().x(),p.point().y(),p.weight()])).collect::<Vec<_>>()
            })).collect::<Vec<_>>();
                    json!({"degree":[s.degree_u(),s.degree_v()],
                "count":[s.control_point_count_u(),s.control_point_count_v()],
                "spans":[s.knots_u().windows(2).filter(|w|w[0]<w[1]).count(),
                         s.knots_v().windows(2).filter(|w|w[0]<w[1]).count()],
                "surface_area":format!("{:?}",s.area(tolerance)), "trims":trims})
                })
                .collect::<Vec<_>>();
        let queries = [[6., 0., 0.], [0., 6., 0.], [5., 0., 1.], [0., 5., 1.]];
        let closest = queries
            .into_iter()
            .map(|p| {
                let q = Point3::try_from(p)?;
                brep.closest_face_parameters(q, tolerance)?
                    .map(|(f, u, v)| {
                        brep.faces()[f]
                            .surface()
                            .evaluate(u, v)
                            .map(|p| p.to_array())
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, viboceros_geometry::GeometryError>>()?;
        let mut swapped = brep.clone();
        swapped.try_swap_face_uv(&(0..brep.faces().len()).collect::<Vec<_>>())?;
        let value = json!({"area_swapped":format!("{:?}",swapped.area(tolerance)),"extend":extend,"vertices":brep.vertices().iter().map(|v|
            v.point().to_array()).collect::<Vec<_>>(), "faces":faces,"closest":closest,
            "area":format!("{:?}",brep.area(tolerance)),
            "area_mass_properties":format!("{:?}",brep.area_mass_properties(tolerance))});
        fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_string_pretty(&value)? + "\n",
        )?;
        println!("{name}: {}", value["area"]);
    }
    Ok(())
}
