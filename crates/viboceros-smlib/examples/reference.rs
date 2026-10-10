//! Emit fixed native measurements for comparison with the licensed Rhino oracle.
use serde_json::{Value, json};
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};
use viboceros_smlib::{BooleanOperation, KernelCurve, Solid};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut results = Vec::<Value>::new();
    for case in [
        "box",
        "box_union",
        "box_intersection",
        "box_difference",
        "cylinder_through_hole",
        "sphere_cavity",
        "hemisphere_pocket",
        "perforated_plate",
    ] {
        let size = if case == "perforated_plate" {
            [12., 12., 1.5]
        } else {
            [10.; 3]
        };
        let mut a = Solid::box_solid(p(0., 0., 0.), size)?;
        match case {
            "box_union" | "box_intersection" | "box_difference" => {
                let b = Solid::box_solid(p(5., 2., 3.), [10.; 3])?;
                let operation = match case {
                    "box_union" => BooleanOperation::Union,
                    "box_intersection" => BooleanOperation::Intersection,
                    _ => BooleanOperation::Difference,
                };
                a = a.boolean(&b, operation)?;
            }
            "cylinder_through_hole" => {
                a = a.boolean(
                    &Solid::cylinder(p(5., 5., -1.), 2., 12.)?,
                    BooleanOperation::Difference,
                )?
            }
            "sphere_cavity" | "hemisphere_pocket" => {
                a = a.boolean(
                    &Solid::sphere(
                        p(5., 5., if case == "sphere_cavity" { 5. } else { 10. }),
                        2.,
                    )?,
                    BooleanOperation::Difference,
                )?
            }
            "perforated_plate" => {
                for x in 0..4 {
                    for y in 0..4 {
                        a = a.boolean(
                            &Solid::cylinder(
                                p(1.5 + 3. * x as f64, 1.5 + 3. * y as f64, -1.),
                                0.75,
                                3.5,
                            )?,
                            BooleanOperation::Difference,
                        )?;
                    }
                }
            }
            _ => {}
        }
        let properties = a.properties(1e-8)?;
        results.push(json!({"id":case,"value":{"volume":properties.volume,"manifold":properties.manifold,"bounds":properties.bounds.map(|p|p.to_array())}}));
    }
    for case in [
        "rational_curve_unit",
        "rational_curve_shifted",
        "rational_polyline",
    ] {
        let (degree, controls, knots) = if case == "rational_polyline" {
            (
                1,
                vec![
                    (p(0., 0., 0.), 1.),
                    (p(2., 0., 0.), 4.),
                    (p(2., 3., 0.), 2.),
                ],
                vec![0., 0., 0.4, 1., 1.],
            )
        } else {
            let (a, b) = if case == "rational_curve_shifted" {
                (100., 105.)
            } else {
                (0., 1.)
            };
            (
                2,
                vec![
                    (p(1., 0., 0.), 1.),
                    (p(1., 1., 0.), std::f64::consts::FRAC_1_SQRT_2),
                    (p(0., 1., 0.), 1.),
                ],
                vec![a, a, a, b, b, b],
            )
        };
        let curve = NurbsCurve::try_new_rational(
            degree,
            controls
                .into_iter()
                .map(|(p, w)| WeightedPoint3::try_new(p, w))
                .collect::<Result<Vec<_>, _>>()?,
            knots,
        )?;
        let native = KernelCurve::from_nurbs(&curve)?;
        let returned = native.to_nurbs()?;
        let domain = returned.domain();
        let samples = (0..=128)
            .map(|i| {
                native
                    .evaluate(domain.start() + (domain.end() - domain.start()) * i as f64 / 128.)
                    .map(|p| p.to_array())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let controls = returned
            .control_points()
            .iter()
            .map(|cp| json!({"point":cp.point().to_array(),"weight":cp.weight()}))
            .collect::<Vec<_>>();
        results.push(json!({"id":case,"value":{"definition":{"degree":returned.degree(),"knots":returned.knots(),"domain":[domain.start(),domain.end()],"control_points":controls},"samples":samples}}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"engine":"smlib_rust_bridge","results":results}))?
    );
    Ok(())
}
