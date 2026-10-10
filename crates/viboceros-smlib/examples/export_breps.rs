//! Export exact native solids as editable 3DM geometry and retain sampled reference data.
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use viboceros_geometry::{Brep, NurbsCurve, Point3, Tolerance};
use viboceros_io::{ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject};
use viboceros_smlib::{BooleanOperation, Solid};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn curve_definition(curve: &NurbsCurve) -> Value {
    let domain = curve.domain();
    let controls = curve
        .control_points()
        .iter()
        .map(|p| {
            json!({
                "point":p.point().to_array(),"weight":p.weight()
            })
        })
        .collect::<Vec<_>>();
    json!({"degree":curve.degree(),"control_points":controls,"knots":curve.knots(),
        "domain":[domain.start(),domain.end()]})
}
fn record(brep: &Brep) -> Value {
    let vertices = brep
        .vertices()
        .iter()
        .map(|v| {
            json!({
                "point":v.point().to_array(),"tolerance":v.tolerance()
            })
        })
        .collect::<Vec<_>>();
    let edges = brep
        .edges()
        .iter()
        .map(|edge| {
            let curve = edge.curve();
            let domain = curve.domain();
            let samples = (0..=32)
                .map(|i| {
                    curve
                        .evaluate(domain.start() + (domain.end() - domain.start()) * i as f64 / 32.)
                        .unwrap()
                        .to_array()
                })
                .collect::<Vec<_>>();
            json!({"tolerance":edge.tolerance(),"curve":{
                "definition":curve_definition(curve),"samples":samples
            }})
        })
        .collect::<Vec<_>>();
    let faces=brep.faces().iter().map(|face| {
        let surface=face.surface();let u=surface.domain_u();let v=surface.domain_v();
        let [u0,u1,v0,v1]=[*u.start(),*u.end(),*v.start(),*v.end()];
        let samples=(0..9).flat_map(|j|(0..9).map(move |i|surface.evaluate(
            u0+(u1-u0)*i as f64/8.,v0+(v1-v0)*j as f64/8.
        ).unwrap().to_array())).collect::<Vec<_>>();
        let loops=face.loops().iter().map(|boundary|boundary.trims().iter().map(|trim| {
            let curve=trim.curve();let domain=curve.domain();
            let controls=curve.control_points().iter().map(|p|json!({
                "point":[p.point().x(),p.point().y()],"weight":p.weight()
            })).collect::<Vec<_>>();
            let definition=json!({"degree":curve.degree(),"control_points":controls,
                "knots":curve.knots(),"domain":[domain.start(),domain.end()]});
            let lifted=(0..=32).map(|i| {
                let uv=curve.evaluate(domain.start()+(domain.end()-domain.start())*i as f64/32.).unwrap();
                surface.evaluate(uv.x(),uv.y()).unwrap().to_array()
            }).collect::<Vec<_>>();
            json!({"definition":definition,"lifted":lifted})
        }).collect::<Vec<_>>()).collect::<Vec<_>>();
        let controls=surface.control_points().iter().map(|p|json!({
            "point":p.point().to_array(),"weight":p.weight()
        })).collect::<Vec<_>>();
        json!({"definition":{"degree":[surface.degree_u(),surface.degree_v()],
            "control_count":[surface.control_point_count_u(),surface.control_point_count_v()],
            "control_points":controls,"knots_u":surface.knots_u(),"knots_v":surface.knots_v(),
            "domain_u":[u.start(),u.end()],"domain_v":[v.start(),v.end()]},
            "samples":samples,"loops":loops})
    }).collect::<Vec<_>>();
    let topology_faces=brep.faces().iter().map(|f|json!({
        "reversed":f.is_reversed(),"loops":f.loops().iter().map(|l|json!({
            "outer":l.loop_type()==viboceros_geometry::BrepLoopType::Outer,
            "trims":l.trims().iter().map(|t|json!({"vertices":t.vertices(),"edge":t.edge(),
                "reversed":t.is_reversed_3d(),"type":format!("{:?}",t.trim_type())})).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    json!({"topology":{"vertices":brep.vertices().len(),"solid":brep.is_solid(),
        "edges":brep.edges().iter().map(|e|e.vertices()).collect::<Vec<_>>(),"faces":topology_faces},
        "vertices":vertices,"edges":edges,"faces":faces})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/viboceros-smlib-breps".into()),
    );
    fs::create_dir_all(&directory)?;
    let a = Solid::box_solid(p(0., 0., 0.), [10.; 3])?;
    let cylinder = Solid::cylinder(p(5., 5., -1.), 2., 12.)?;
    let sphere = Solid::sphere(p(5., 5., 10.), 2.)?;
    let other = Solid::box_solid(p(5., 2., 3.), [10.; 3])?;
    let mut cases = vec![
        ("box", a.boolean(&a, BooleanOperation::Union)?),
        ("cylinder", cylinder),
        ("sphere", sphere),
    ];
    cases.push((
        "through_hole",
        a.boolean(&cases[1].1, BooleanOperation::Difference)?,
    ));
    cases.push((
        "pocket",
        a.boolean(&cases[2].1, BooleanOperation::Difference)?,
    ));
    cases.push((
        "cavity",
        a.boolean(
            &Solid::sphere(p(5., 5., 5.), 2.)?,
            BooleanOperation::Difference,
        )?,
    ));
    for (name, operation) in [
        ("box_union", BooleanOperation::Union),
        ("box_intersection", BooleanOperation::Intersection),
        ("box_difference", BooleanOperation::Difference),
    ] {
        cases.push((name, a.boolean(&other, operation)?));
    }
    let mut plate = Solid::box_solid(p(0., 0., 0.), [12., 12., 1.5])?;
    for x in 0..4 {
        for y in 0..4 {
            plate = plate.boolean(
                &Solid::cylinder(p(1.5 + 3. * x as f64, 1.5 + 3. * y as f64, -1.), 0.75, 3.5)?,
                BooleanOperation::Difference,
            )?;
        }
    }
    cases.push(("plate", plate));
    let mut references = Vec::new();
    let mut requests = Vec::new();
    for (name, solid) in cases {
        let brep = solid.to_brep(Tolerance::DEFAULT)?;
        let path = directory.join(format!("{name}.3dm"));
        let model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Default".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![ThreeDmObject::new(ThreeDmGeometry::Brep(brep.clone()), 0)],
        );
        viboceros_io::write_3dm_file(&path, &model)?;
        references.push(json!({"id":name,"value":record(&brep)}));
        requests.push(json!({"op":"three_dm_brep_interchange","id":name,"artifact_path":path.canonicalize()?}));
        let mut step = Vec::new();
        let step_result = viboceros_io::write_step_nurbs_breps(&mut step, [&brep]);
        match step_result {
            Ok(()) => fs::write(directory.join(format!("{name}.step")), step)?,
            Err(error) => eprintln!("{name}: STEP remains unsupported: {error}"),
        }
        println!(
            "{name}: {} faces, {} edges, {} vertices, volume {:.12}",
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len(),
            brep.signed_volume(Tolerance::DEFAULT)?
        );
    }
    fs::write(
        directory.join("local.json"),
        serde_json::to_string_pretty(&json!({"results":references}))?,
    )?;
    fs::write(
        directory.join("request.json"),
        serde_json::to_string_pretty(
            &json!({"protocol_version":1,"iterations":1,"operations":requests}),
        )?,
    )?;
    Ok(())
}
