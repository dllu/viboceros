//! Bounded independent parallel-silhouette recipes; native Rhino capture pending.
use super::*;

pub(super) fn run(
    iterations: u32,
    case: &str,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let frame = viboceros_command::CommandContext::default().construction_plane;
    let (source, direction, internal) = match case {
        "box_top" | "box_bottom" | "box_oblique" | "box_internal" => (
            Geometry::Brep(Brep::try_box(
                frame,
                [[0., 2.], [0., 3.], [0., 4.]],
                tolerance,
            )?),
            match case {
                "box_bottom" => [0., 0., -1.],
                "box_oblique" | "box_internal" => [1., 2., 3.],
                _ => [0., 0., 1.],
            },
            case == "box_internal",
        ),
        "sphere_top" | "sphere_oblique" => (
            Geometry::Brep(Brep::try_surface_grid(
                &NurbsSurface::try_sphere(frame, 2.)?,
                &[],
                &[],
                tolerance,
            )?),
            if case == "sphere_top" {
                [0., 0., 1.]
            } else {
                [1., 2., 3.]
            },
            false,
        ),
        "torus_top" => (
            Geometry::Brep(Brep::try_surface_grid(
                &NurbsSurface::try_torus(frame, 3., 1.)?,
                &[],
                &[],
                tolerance,
            )?),
            [0., 0., 1.],
            false,
        ),
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "unknown parallel silhouette recipe",
            ));
        }
    };
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        doc.select_objects_direct([id], SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "Silhouette Direction={},{},{} InternalEdges={}",
                direction[0],
                direction[1],
                direction[2],
                if internal { "Yes" } else { "No" },
            ),
        )?;
        let mut curves = Vec::new();
        for obj in doc.selected_objects().filter(|obj| obj.id() != id) {
            let Geometry::NurbsCurve(curve) = obj.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "silhouette output is not a curve",
                ));
            };
            curves.push(json!({
                "length": curve.length(tolerance)?,
                "samples": (0..=32).map(|i| {
                    curve.evaluate(curve.parameter_at(f64::from(i)/32.)?).map(Point3::to_array)
                }).collect::<Result<Vec<_>, _>>()?,
                "definition": nurbs_curve_definition_value(curve),
            }));
        }
        let total_length = curves
            .iter()
            .map(|v| v["length"].as_f64().unwrap())
            .sum::<f64>();
        Ok(json!({"curves":curves,"total_length":total_length}))
    })
}

#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn silhouette_pending_recipes_exercise_native_geometry_and_internal_edges() {
        let tolerance = Tolerance::try_new(1e-5, 1e-12, 0.05).unwrap();
        let mut counts = Vec::new();
        for case in [
            "box_top",
            "box_bottom",
            "box_oblique",
            "box_internal",
            "sphere_top",
            "sphere_oblique",
            "torus_top",
        ] {
            let (value, _) = run(1, case, tolerance).unwrap();
            counts.push(value["curves"].as_array().unwrap().len());
            let expected = match case {
                "box_top" | "box_bottom" => Some(10.),
                "sphere_top" | "sphere_oblique" => Some(4. * std::f64::consts::PI),
                "torus_top" => Some(12. * std::f64::consts::PI),
                _ => None,
            };
            if let Some(length) = expected {
                assert!(
                    (value["total_length"].as_f64().unwrap() - length).abs() < 1e-3,
                    "{case}: {value}"
                );
            }
        }
        assert_eq!(&counts[..2], &[4, 4]);
        assert!(
            counts[3] > counts[2],
            "InternalEdges must retain additional visible box features: {counts:?}"
        );
        assert!(run(1, "unknown", tolerance).is_err());
    }
}
