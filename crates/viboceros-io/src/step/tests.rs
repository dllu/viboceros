use monstertruck::core::cgmath64::SquareMatrix;
use std::io::Cursor;

use super::*;

#[test]
fn nurbs_brep_step_export_retains_curved_surface_and_explicit_pcurves() {
    use viboceros_geometry::{Brep, Frame3, NurbsSurface, Vector3};
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let surface = NurbsSurface::try_bilinear([
        point(0., 0., 0.),
        point(2., 0., 0.),
        point(2., 3., 1.),
        point(0., 3., 0.),
    ])
    .unwrap();
    let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    assert!(
        source.faces()[0]
            .surface()
            .plane(Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    let mut output = Vec::new();
    write_step_nurbs_breps(&mut output, [&source]).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("B_SPLINE_SURFACE_WITH_KNOTS("));
    assert_eq!(text.matches("PCURVE(").count(), source.edges().len());
    let table = Table::from_step(&text).unwrap();
    assert_eq!(table.entity_report.total(), 0);
    let shell_id = *table.shell.keys().next().unwrap();
    let (shell, report) = reported_trimmed_shell(&table, shell_id).unwrap();
    assert_eq!(report.total_lost(), 0);
    assert_eq!(shell.faces.len(), 1);
    assert_eq!(shell.edges.len(), 4);
    assert!(
        shell.faces[0].boundaries[0]
            .iter()
            .all(|edge| edge.trim_curve.is_some())
    );
    let frame = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let box_brep = Brep::try_box(frame, [[-2., 2.]; 3], Tolerance::DEFAULT).unwrap();
    let mut mixed = Vec::new();
    write_step_native_breps_in_units(
        &mut mixed,
        [&box_brep, &source],
        &LengthUnitSystem::Millimeters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mixed = String::from_utf8(mixed).unwrap();
    assert_eq!(mixed.matches("MANIFOLD_SOLID_BREP(").count(), 1);
    assert_eq!(mixed.matches("SHELL_BASED_SURFACE_MODEL(").count(), 1);
    assert!(mixed.contains("B_SPLINE_SURFACE("));
    assert_eq!(mixed.matches("PLANE(").count(), 6);
    assert_eq!(mixed.matches("LINE(").count(), 12);
    assert_eq!(mixed.matches("PCURVE(").count(), 4);
    let table = Table::from_step(&mixed).unwrap();
    assert_eq!(table.entity_report.total(), 0);
    let box_shell_id = *table
        .shell
        .keys()
        .find(|&&id| reported_trimmed_shell(&table, id).unwrap().0.faces.len() == 6)
        .unwrap();
    let restored = native_planar::convert_shell(&table, box_shell_id, Tolerance::DEFAULT).unwrap();
    assert!((restored.signed_volume(Tolerance::DEFAULT).unwrap() - 64.).abs() < 1e-9);
    let native = read_step_native_instances(Cursor::new(mixed), Tolerance::DEFAULT).unwrap();
    assert_eq!(native.instances.len(), 2);
    assert!(
        native
            .instances
            .iter()
            .any(|instance| instance.brep.faces().len() == 6)
    );
    assert!(native.instances.iter().any(|instance| {
        instance.brep.faces().len() == 1
            && instance.brep.faces()[0]
                .surface()
                .plane(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
    }));
}

#[test]
fn nurbs_brep_step_export_keeps_curved_edges_and_surface_shape() {
    use monstertruck::meshing::prelude::{ParametricCurve, ParametricSurface};
    use viboceros_geometry::{Brep, NurbsSurface};
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            point(0., 0., 0.),
            point(1., 1., 0.),
            point(2., 0., 0.),
            point(0., 0., 3.),
            point(1., 1., 3.),
            point(2., 0., 3.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        source
            .edges()
            .iter()
            .filter(|edge| edge.curve().degree() == 2)
            .count(),
        2
    );
    let mut output = Vec::new();
    write_step_nurbs_breps(&mut output, [&source]).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("B_SPLINE_CURVE(2,"));
    let table = Table::from_step(&text).unwrap();
    assert_eq!(table.entity_report.total(), 0);
    let shell_id = *table.shell.keys().next().unwrap();
    let (shell, report) = reported_trimmed_shell(&table, shell_id).unwrap();
    assert_eq!(report.total_lost(), 0);
    assert_eq!(shell.faces.len(), 1);
    assert_eq!(shell.edges.len(), 4);
    for (u, v) in [(0.25, 0.25), (0.5, 0.5), (0.75, 0.8)] {
        let expected = source.faces()[0].surface().evaluate(u, v).unwrap();
        let actual = shell.faces[0].surface.evaluate(u, v);
        assert!((actual.x - expected.x()).abs() < 1e-12);
        assert!((actual.y - expected.y()).abs() < 1e-12);
        assert!((actual.z - expected.z()).abs() < 1e-12);
    }
    for (source_edge, loaded_edge) in source.edges().iter().zip(&shell.edges) {
        for (source_vertex, loaded_vertex) in source_edge
            .vertices()
            .into_iter()
            .zip([loaded_edge.vertices.0, loaded_edge.vertices.1])
        {
            let expected = source.vertices()[source_vertex].point();
            let actual = shell.vertices[loaded_vertex];
            assert!((actual.x - expected.x()).abs() < 1e-12);
            assert!((actual.y - expected.y()).abs() < 1e-12);
            assert!((actual.z - expected.z()).abs() < 1e-12);
        }
        if source_edge.curve().degree() != 2 {
            continue;
        }
        let domain = source_edge.curve().domain();
        for fraction in [0., 0.25, 0.5, 0.75, 1.] {
            let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
            let expected = source_edge.curve().evaluate(t).unwrap();
            let actual = loaded_edge.curve.evaluate(t);
            assert!((actual.x - expected.x()).abs() < 1e-12);
            assert!((actual.y - expected.y()).abs() < 1e-12);
            assert!((actual.z - expected.z()).abs() < 1e-12);
        }
    }

    let native = read_step_native_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(native.instances.len(), 1);
    let restored = &native.instances[0].brep;
    assert_eq!(restored.faces().len(), 1);
    assert_eq!(restored.edges().len(), 4);
    assert_eq!(restored.faces()[0].surface().degree_u(), 2);
    for (u, v) in [(0.25, 0.25), (0.5, 0.5), (0.75, 0.8)] {
        let expected = source.faces()[0].surface().evaluate(u, v).unwrap();
        let actual = restored.faces()[0].surface().evaluate(u, v).unwrap();
        assert!(actual.distance_to(expected).unwrap() < 1e-12);
    }

    let mut scaled_output = Vec::new();
    write_step_nurbs_breps_in_units(
        &mut scaled_output,
        [&source],
        &LengthUnitSystem::Inches,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let scaled_text = String::from_utf8(scaled_output).unwrap();
    let scaled_table = Table::from_step(&scaled_text).unwrap();
    let scaled_shell_id = *scaled_table.shell.keys().next().unwrap();
    let (scaled_shell, scaled_report) =
        reported_trimmed_shell(&scaled_table, scaled_shell_id).unwrap();
    assert_eq!(scaled_report.total_lost(), 0);
    let actual = scaled_shell.faces[0].surface.evaluate(0.5, 0.5);
    let expected = source.faces()[0].surface().evaluate(0.5, 0.5).unwrap();
    for (a, b) in [actual.x, actual.y, actual.z]
        .into_iter()
        .zip(expected.to_array())
    {
        assert!((a - b * 25.4).abs() < 1e-12);
    }
}

#[test]
fn nurbs_brep_step_export_rejects_periodic_seam_atomically() {
    use viboceros_geometry::{Brep, Frame3, NurbsSurface, Vector3};
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = NurbsSurface::try_cylinder(frame, 2., 0., 3.).unwrap();
    let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    assert_eq!(source.faces().len(), 1);
    assert!(source.faces()[0].loops()[0].trims().len() > source.edges().len());
    let mut output = Vec::new();
    assert!(matches!(
        write_step_nurbs_breps(&mut output, [&source]),
        Err(StepError::UnsupportedNativeBrep { .. })
    ));
    assert!(output.is_empty());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.step");
    std::fs::write(&path, b"old STEP bytes").unwrap();
    assert!(matches!(
        write_step_native_breps_file_in_units(
            &path,
            [&source],
            &LengthUnitSystem::Millimeters,
            Tolerance::DEFAULT,
        ),
        Err(StepError::UnsupportedNativeBrep { .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"old STEP bytes");
}

#[test]
fn nurbs_brep_step_export_keeps_open_rational_arc_surface() {
    use monstertruck::meshing::prelude::ParametricSurface;
    use viboceros_geometry::{Brep, NurbsSurface, WeightedPoint3};
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let middle = std::f64::consts::FRAC_1_SQRT_2;
    let controls = [
        (point(2., 0., 0.), 1.),
        (point(2., 2., 0.), middle),
        (point(0., 2., 0.), 1.),
        (point(2., 0., 3.), 1.),
        (point(2., 2., 3.), middle),
        (point(0., 2., 3.), 1.),
    ]
    .into_iter()
    .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
    .collect();
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    let mut output = Vec::new();
    write_step_nurbs_breps(&mut output, [&source]).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("RATIONAL_B_SPLINE_SURFACE("));
    assert!(text.contains("RATIONAL_B_SPLINE_CURVE("));
    let table = Table::from_step(&text).unwrap();
    assert_eq!(table.entity_report.total(), 0);
    let shell_id = *table.shell.keys().next().unwrap();
    let (shell, report) = reported_trimmed_shell(&table, shell_id).unwrap();
    assert_eq!(report.total_lost(), 0);
    assert!(
        shell.faces[0].boundaries[0]
            .iter()
            .all(|use_| use_.trim_curve.is_some())
    );
    let actual = shell.faces[0].surface.evaluate(0.5, 0.5);
    let expected = source.faces()[0].surface().evaluate(0.5, 0.5).unwrap();
    assert!((actual.x - expected.x()).abs() < 1e-12);
    assert!((actual.y - expected.y()).abs() < 1e-12);
    assert!((actual.z - expected.z()).abs() < 1e-12);
    let native = read_step_native_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    let restored = &native.instances[0].brep;
    assert_eq!(restored.faces().len(), 1);
    assert!(restored.faces()[0].surface().is_rational());
    let restored_midpoint = restored.faces()[0].surface().evaluate(0.5, 0.5).unwrap();
    assert!(restored_midpoint.distance_to(expected).unwrap() < 1e-12);
}

#[test]
fn explicit_linear_bspline_step_pcurves_import_without_losing_parameter_intervals() {
    let original = polygon_face_step(&[vec![[0., 0.], [10., 0.], [0., 10.]]], false);
    let table = Table::from_step(&original).unwrap();
    let plane = *table.plane.keys().next().unwrap();
    let mut ids = table.edge_curve.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    let mut replacements = std::collections::BTreeMap::new();
    let mut records = String::from(
        "#99999 = (GEOMETRIC_REPRESENTATION_CONTEXT(2) REPRESENTATION_CONTEXT('',''));\n",
    );
    for (index, edge_id) in ids.into_iter().enumerate() {
        let edge = &table.edge_curve[&edge_id];
        assert!(edge.same_sense);
        let start = referenced_entity(&edge.edge_start, "start").unwrap();
        let end = referenced_entity(&edge.edge_end, "end").unwrap();
        let geometry = referenced_entity(&edge.edge_geometry, "geometry").unwrap();
        let coordinates = |vertex| {
            let point =
                referenced_entity(&table.vertex_point[&vertex].vertex_geometry, "point").unwrap();
            table.cartesian_point[&point].coordinates.clone()
        };
        let a = coordinates(start);
        let b = coordinates(end);
        let base = 100000 + index * 10;
        replacements.insert(
            format!("#{edge_id} ="),
            format!("#{edge_id} = EDGE_CURVE('', #{start}, #{end}, #{base}, .T.);"),
        );
        records.push_str(&format!(
            "#{base} = SURFACE_CURVE('', #{geometry}, (#{pcurve}), .CURVE_3D.);\n#{pcurve} = PCURVE('', #{plane}, #{representation});\n#{representation} = DEFINITIONAL_REPRESENTATION('', (#{spline}), #99999);\n#{spline} = B_SPLINE_CURVE_WITH_KNOTS('', 1, (#{p0}, #{p1}), .UNSPECIFIED., .F., .F., (2,2), (-3.,7.), .UNSPECIFIED.);\n#{p0} = CARTESIAN_POINT('', ({ax:?},{ay:?}));\n#{p1} = CARTESIAN_POINT('', ({bx:?},{by:?}));\n",
            pcurve=base+1, representation=base+2, spline=base+3, p0=base+4, p1=base+5,
            ax=a[0], ay=a[1], bx=b[0], by=b[1],
        ));
    }
    let mut text = original
        .lines()
        .map(|line| {
            replacements
                .iter()
                .find(|(prefix, _)| line.starts_with(prefix.as_str()))
                .map_or_else(|| line.to_owned(), |(_, replacement)| replacement.clone())
        })
        .collect::<Vec<_>>()
        .join("\n");
    text.insert_str(text.rfind("ENDSEC;").unwrap(), &records);
    // Give the rational UV curve a matching rational 3D parameterization.
    // The loader checks sampled parameter correspondence before retaining p-curves.
    let mut rational_edges = std::collections::BTreeMap::new();
    let mut polyline_edges = std::collections::BTreeMap::new();
    for edge in table.edge_curve.values() {
        let geometry = referenced_entity(&edge.edge_geometry, "geometry").unwrap();
        let control = |vertex| {
            let id = referenced_entity(vertex, "vertex").unwrap();
            referenced_entity(&table.vertex_point[&id].vertex_geometry, "point").unwrap()
        };
        let start = control(&edge.edge_start);
        let end = control(&edge.edge_end);
        polyline_edges.insert(
            format!("#{geometry} ="),
            format!("#{geometry} = POLYLINE('', (#{start},#{end}));"),
        );
        rational_edges.insert(format!("#{geometry} ="), format!("#{geometry} = (BOUNDED_CURVE() B_SPLINE_CURVE(1,(#{start},#{end}),.UNSPECIFIED.,.F.,.F.) B_SPLINE_CURVE_WITH_KNOTS((2,2),(-3.,7.),.UNSPECIFIED.) CURVE() GEOMETRIC_REPRESENTATION_ITEM() RATIONAL_B_SPLINE_CURVE((1.,4.)) REPRESENTATION_ITEM(''));"));
    }
    let rational_text = text.lines().map(|line| {
        if let Some((_, replacement)) = rational_edges.iter().find(|(prefix, _)| line.starts_with(prefix.as_str())) { return replacement.clone(); }
        if !line.contains(" = B_SPLINE_CURVE_WITH_KNOTS(") { return line.to_owned(); }
        let ids = line.split('#').skip(1).map(|part| {
            part.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u64>().unwrap()
        }).collect::<Vec<_>>();
        assert_eq!(ids.len(), 3);
        format!("#{} = (BOUNDED_CURVE() B_SPLINE_CURVE(1,(#{},#{}),.UNSPECIFIED.,.F.,.F.) B_SPLINE_CURVE_WITH_KNOTS((2,2),(-3.,7.),.UNSPECIFIED.) CURVE() GEOMETRIC_REPRESENTATION_ITEM() RATIONAL_B_SPLINE_CURVE((1.,4.)) REPRESENTATION_ITEM(''));", ids[0], ids[1], ids[2])
    }).collect::<Vec<_>>().join("\n");
    let polyline_text = text
        .lines()
        .map(|line| {
            if let Some((_, replacement)) = polyline_edges
                .iter()
                .find(|(prefix, _)| line.starts_with(prefix.as_str()))
            {
                return replacement.clone();
            }
            if !line.contains(" = B_SPLINE_CURVE_WITH_KNOTS(") {
                return line.to_owned();
            }
            let ids = line
                .split('#')
                .skip(1)
                .map(|part| {
                    part.chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<u64>()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(ids.len(), 3);
            format!("#{} = POLYLINE('', (#{},#{}));", ids[0], ids[1], ids[2])
        })
        .collect::<Vec<_>>()
        .join("\n");
    for (text, encoding) in [
        (text, "bspline"),
        (rational_text, "rational"),
        (polyline_text, "polyline"),
    ] {
        let rational = encoding == "rational";
        let polyline = encoding == "polyline";
        let parsed = Table::from_step(&text).unwrap();
        assert_eq!(parsed.entity_report.total(), 0);
        let shell = *parsed.shell.keys().next().unwrap();
        let (loaded, report) = reported_trimmed_shell(&parsed, shell).unwrap();
        assert_eq!(report.total_lost(), 0);
        for edge in loaded.faces[0].boundaries.iter().flatten() {
            use monstertruck::step::load::step_geometry::Curve2D;
            let curve = edge.trim_curve.as_ref().unwrap().curve().as_ref();
            assert!(
                if rational {
                    matches!(curve, Curve2D::NurbsCurve(_))
                } else if polyline {
                    matches!(curve, Curve2D::Polyline(_))
                } else {
                    matches!(curve, Curve2D::BsplineCurve(_))
                },
                "encoding={encoding}: {curve:?}"
            );
        }
        let native = read_step_planar_shells(Cursor::new(text), Tolerance::DEFAULT).unwrap();
        let brep = &native[0].brep;
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 50.).abs() < 1e-10);
        for trim in brep.faces()[0].loops()[0].trims() {
            assert_eq!(
                trim.curve().domain(),
                if polyline { 0.0..=1.0 } else { -3.0..=7.0 }
            );
            if rational {
                let mut weights = trim
                    .curve()
                    .control_points()
                    .iter()
                    .map(|p| p.weight())
                    .collect::<Vec<_>>();
                weights.sort_by(f64::total_cmp);
                assert_eq!(weights, [1., 4.]);
            }
        }
        if rational {
            for edge in brep.edges() {
                assert_eq!(edge.curve().domain(), -3.0..=7.0);
                assert_eq!(
                    edge.curve()
                        .control_points()
                        .iter()
                        .map(|p| p.weight())
                        .collect::<Vec<_>>(),
                    [1., 4.]
                );
            }
        }
        assert_planar_brep_archive_round_trip(brep);
    }
}

fn assert_planar_brep_archive_round_trip(
    brep: &viboceros_geometry::Brep,
) -> viboceros_geometry::Brep {
    use crate::{
        ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file, write_3dm_file,
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native STEP face.3dm");
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "STEP".into(),
            color: [10, 20, 30],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(ThreeDmGeometry::Brep(brep.clone()), 0)],
    );
    write_3dm_file(&path, &model).unwrap();
    let mut restored = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(restored.unsupported_object_count(), 0);
    assert_eq!(restored.objects.len(), 1);
    let ThreeDmGeometry::Brep(actual) = restored.objects.pop().unwrap().geometry else {
        panic!("lost B-rep")
    };
    assert_eq!(
        (
            actual.vertices().len(),
            actual.edges().len(),
            actual.faces().len()
        ),
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        )
    );
    assert!(
        (actual.area(Tolerance::DEFAULT).unwrap() - brep.area(Tolerance::DEFAULT).unwrap()).abs()
            < 1e-9
    );
    for (vertex, old) in actual.vertices().iter().zip(brep.vertices()) {
        assert_eq!(vertex.point(), old.point());
    }
    for (edge, old) in actual.edges().iter().zip(brep.edges()) {
        assert_eq!(edge.vertices(), old.vertices());
        assert_eq!(edge.curve(), old.curve());
    }
    for (face, old) in actual.faces().iter().zip(brep.faces()) {
        assert_eq!(face.surface(), old.surface());
        assert_eq!(face.is_reversed(), old.is_reversed());
        assert_eq!(face.loops().len(), old.loops().len());
        for (boundary, old_boundary) in face.loops().iter().zip(old.loops()) {
            assert_eq!(boundary.loop_type(), old_boundary.loop_type());
            assert_eq!(boundary.trims().len(), old_boundary.trims().len());
            for (trim, old_trim) in boundary.trims().iter().zip(old_boundary.trims()) {
                assert_eq!(trim.vertices(), old_trim.vertices());
                assert_eq!(trim.edge(), old_trim.edge());
                assert_eq!(trim.curve(), old_trim.curve());
                assert_eq!(trim.is_reversed_3d(), old_trim.is_reversed_3d());
                assert_eq!(trim.trim_type(), old_trim.trim_type());
            }
        }
    }
    actual
}

#[test]
fn planar_hole_trims_survive_loading_and_native_conversion() {
    let outer = vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]];
    let inner = vec![[2., 2.], [2., 4.], [4., 4.], [4., 2.]];
    for inner_first in [false, true] {
        for reversed in [false, true] {
            let boundaries = if inner_first {
                vec![inner.clone(), outer.clone()]
            } else {
                vec![outer.clone(), inner.clone()]
            };
            let text = polygon_face_step(&boundaries, reversed);
            let table = Table::from_step(&text).unwrap();
            let id = *table.shell.keys().next().unwrap();
            let (loaded, report) = reported_trimmed_shell(&table, id).unwrap();
            assert_eq!(report.total_lost(), 0);
            assert_eq!(
                (
                    loaded.vertices.len(),
                    loaded.edges.len(),
                    loaded.faces.len()
                ),
                (8, 8, 1)
            );
            let face = &loaded.faces[0];
            assert_eq!(face.orientation, !reversed);
            assert_eq!(face.boundaries.len(), 2);
            let areas = face
                .boundaries
                .iter()
                .map(|boundary| {
                    assert_eq!(boundary.len(), 4);
                    boundary
                        .iter()
                        .map(|edge_use| {
                            let trim = edge_use.trim_curve.as_ref().expect("exact planar trim");
                            let (start, end) = trim.range_tuple();
                            let a = trim.curve().evaluate(start);
                            let b = trim.curve().evaluate(end);
                            (a.x * b.y - a.y * b.x) * 0.5
                        })
                        .sum::<f64>()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                areas,
                if inner_first {
                    vec![-4., 100.]
                } else {
                    vec![100., -4.]
                }
            );
            let native = read_step_planar_shells(Cursor::new(&text), Tolerance::DEFAULT).unwrap();
            let brep = &native[0].brep;
            assert_eq!(
                (
                    brep.vertices().len(),
                    brep.edges().len(),
                    brep.faces().len()
                ),
                (8, 8, 1)
            );
            assert_eq!(brep.faces()[0].is_reversed(), reversed);
            assert_eq!(
                brep.faces()[0].loops()[0].loop_type(),
                viboceros_geometry::BrepLoopType::Outer
            );
            assert_eq!(
                brep.faces()[0].loops()[1].loop_type(),
                viboceros_geometry::BrepLoopType::Inner
            );
            assert!((brep.area(Tolerance::DEFAULT).unwrap() - 96.0).abs() < 1e-10);
            assert!(brep.signed_volume(Tolerance::DEFAULT).is_err());
            let archived = assert_planar_brep_archive_round_trip(brep);
            assert!(
                archived.faces()[0].loops()[1]
                    .trims()
                    .iter()
                    .all(|trim| matches!(
                        trim.iso(),
                        viboceros_geometry::SurfaceIso::InteriorUConstant
                            | viboceros_geometry::SurfaceIso::InteriorVConstant
                    ))
            );
        }
    }
}

fn polygon_face_step(boundaries: &[Vec<[f64; 2]>], reversed: bool) -> String {
    use monstertruck::modeling::{Edge, Face, Plane, Shell, Vertex, Wire, builder};
    let boundaries = boundaries
        .iter()
        .map(|points| {
            let vertices = points
                .iter()
                .map(|p| Vertex::new(TruckPoint3::new(p[0], p[1], 0.)))
                .collect::<Vec<_>>();
            Wire::from(
                (0..points.len())
                    .map(|i| builder::line(&vertices[i], &vertices[(i + 1) % points.len()]))
                    .collect::<Vec<Edge>>(),
            )
        })
        .collect::<Vec<_>>();
    let mut face = Face::new(
        boundaries,
        Plane::new(
            TruckPoint3::new(0., 0., 0.),
            TruckPoint3::new(1., 0., 0.),
            TruckPoint3::new(0., 1., 0.),
        )
        .into(),
    );
    if reversed {
        face.invert();
    }
    let shell = Shell::from(vec![face]).compress();
    CompleteStepDisplay::new(
        TruckStepModel::from(&shell),
        StepHeaderDescriptor::default(),
    )
    .to_string()
}

#[test]
fn native_step_imports_bspline_faces_with_polygon_holes() {
    use viboceros_geometry::{Brep, BrepFace, NurbsSurface};
    let outer = vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]];
    let hole = vec![[2., 2.], [2., 4.], [4., 4.], [4., 2.]];
    for reversed in [false, true] {
        let planar = polygon_face_step(&[hole.clone(), outer.clone()], reversed);
        let imported = read_step_planar_instances(Cursor::new(planar), Tolerance::DEFAULT).unwrap();
        let source = &imported.instances[0].brep;
        let surface = NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            [
                [0., 0.],
                [5., 0.],
                [10., 0.],
                [0., 10.],
                [5., 10.],
                [10., 10.],
            ]
            .into_iter()
            .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap())
            .collect(),
            vec![0., 0., 0., 10., 10., 10.],
            vec![0., 0., 10., 10.],
        )
        .unwrap();
        let face =
            BrepFace::try_new(surface, reversed, source.faces()[0].loops().to_vec()).unwrap();
        let source = Brep::try_new(
            source.vertices().to_vec(),
            source.edges().to_vec(),
            vec![face],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut output = Vec::new();
        write_step_nurbs_breps(&mut output, [&source]).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("B_SPLINE_SURFACE_WITH_KNOTS("));
        assert_eq!(text.matches("PCURVE(").count(), 8);
        let table = Table::from_step(&text).unwrap();
        assert_eq!(table.entity_report.total(), 0);
        assert!(matches!(
            read_step_planar_instances(Cursor::new(&text), Tolerance::DEFAULT),
            Err(StepError::UnsupportedPlanarShell { .. })
        ));
        let native = read_step_native_instances(Cursor::new(&text), Tolerance::DEFAULT).unwrap();
        let brep = &native.instances[0].brep;
        assert_eq!(brep.faces()[0].loops().len(), 2);
        assert_eq!(brep.faces()[0].is_reversed(), reversed);
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 96.).abs() < 1e-9);
    }
}

#[test]
fn native_step_imports_analytic_circle_edge_and_uv_trim() {
    use monstertruck::modeling::{Plane, builder};
    use monstertruck::step::load::step_geometry::{
        Conic2D, Conic3D, Curve2D, Curve3D, ElementarySurface, Surface,
    };
    use monstertruck::topology::{Edge, Face, Shell, Vertex, Wire};
    for angle in [
        std::f64::consts::FRAC_PI_2,
        3. * std::f64::consts::FRAC_PI_2,
        35. * std::f64::consts::PI / 18.,
    ] {
        let origin = Vertex::new(TruckPoint3::new(0., 0., 0.));
        let east = Vertex::new(TruckPoint3::new(2., 0., 0.));
        let north = Vertex::new(TruckPoint3::new(2. * angle.cos(), 2. * angle.sin(), 0.));
        let arc_midpoint = TruckPoint3::new(2. * (angle / 2.).cos(), 2. * (angle / 2.).sin(), 0.);
        let edges: Vec<Edge<TruckPoint3, Curve3D>> = vec![
            builder::line(&origin, &east),
            builder::circle_arc(&east, &north, arc_midpoint),
            builder::line(&north, &origin),
        ];
        let wire = Wire::from(edges);
        let surface = Surface::ElementarySurface(ElementarySurface::Plane(Plane::new(
            TruckPoint3::new(0., 0., 0.),
            TruckPoint3::new(1., 0., 0.),
            TruckPoint3::new(0., 1., 0.),
        )));
        let face = Face::new(vec![wire], surface);
        let shell = Shell::from(vec![face]).compress();
        let text = CompleteStepDisplay::new(
            TruckStepModel::from(&shell),
            StepHeaderDescriptor::default(),
        )
        .to_string();
        assert!(text.contains("CIRCLE("));
        let table = Table::from_step(&text).unwrap();
        assert_eq!(table.entity_report.total(), 0);
        let shell_id = *table.shell.keys().next().unwrap();
        let (decoded, report) = reported_trimmed_shell(&table, shell_id).unwrap();
        assert_eq!(report.total_lost(), 0);
        assert!(
        decoded
            .edges
            .iter()
            .any(|edge| matches!(&edge.curve, Curve3D::SurfaceCurve(curve) if matches!(curve.leader(), Curve3D::Conic(Conic3D::Ellipse(_)))))
    );
        assert!(decoded.faces[0].boundaries[0].iter().any(|edge| matches!(
            edge.trim_curve.as_ref().map(|curve| curve.curve().as_ref()),
            Some(Curve2D::Conic(Conic2D::Ellipse(_)))
        )));
        assert!(matches!(
            read_step_planar_instances(Cursor::new(&text), Tolerance::DEFAULT),
            Err(StepError::UnsupportedPlanarShell { .. })
        ));
        let native = read_step_native_instances(Cursor::new(text), Tolerance::DEFAULT)
            .unwrap_or_else(|error| panic!("angle {angle}: {error:?}"));
        assert_eq!(native.instances.len(), 1);
        let brep = &native.instances[0].brep;
        assert_eq!(
            (
                brep.vertices().len(),
                brep.edges().len(),
                brep.faces().len()
            ),
            (3, 3, 1)
        );
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 2. * angle).abs() < 1e-9);
        let arc = brep
            .edges()
            .iter()
            .find(|edge| edge.curve().degree() == 2)
            .unwrap();
        for t in [0., 0.25, 0.5, 0.75, 1.] {
            let p = arc.curve().evaluate(t).unwrap();
            assert!(((p.x() * p.x() + p.y() * p.y()).sqrt() - 2.).abs() < 1e-12);
        }
    }
}

#[test]
fn native_step_imports_analytic_ellipse_arc() {
    use monstertruck::modeling::{Plane, Transformed, builder};
    use monstertruck::step::load::step_geometry::{Conic3D, Curve3D, ElementarySurface, Surface};
    use monstertruck::topology::{Edge, Face, Shell, Vertex, Wire};
    let origin = Vertex::new(TruckPoint3::new(0., 0., 0.));
    let east = Vertex::new(TruckPoint3::new(2., 0., 0.));
    let circular_north = Vertex::new(TruckPoint3::new(0., 2., 0.));
    let elliptical_north = Vertex::new(TruckPoint3::new(0., 1., 0.));
    let circular_arc: Edge<TruckPoint3, Curve3D> = builder::circle_arc(
        &east,
        &circular_north,
        TruckPoint3::new(std::f64::consts::SQRT_2, std::f64::consts::SQRT_2, 0.),
    );
    let Curve3D::Conic(Conic3D::Ellipse(mut ellipse)) = circular_arc.oriented_curve().clone()
    else {
        panic!("builder did not produce an analytic circle")
    };
    ellipse.transform_by(Matrix4::from_nonuniform_scale(1., 0.5, 1.));
    let edges: Vec<Edge<TruckPoint3, Curve3D>> = vec![
        builder::line(&origin, &east),
        Edge::new(
            &east,
            &elliptical_north,
            Curve3D::Conic(Conic3D::Ellipse(ellipse)),
        ),
        builder::line(&elliptical_north, &origin),
    ];
    let surface = Surface::ElementarySurface(ElementarySurface::Plane(Plane::new(
        TruckPoint3::new(0., 0., 0.),
        TruckPoint3::new(1., 0., 0.),
        TruckPoint3::new(0., 1., 0.),
    )));
    let shell = Shell::from(vec![Face::new(vec![Wire::from(edges)], surface)]).compress();
    let text = CompleteStepDisplay::new(
        TruckStepModel::from(&shell),
        StepHeaderDescriptor::default(),
    )
    .to_string();
    assert!(text.contains("ELLIPSE("));
    let native = read_step_native_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    let brep = &native.instances[0].brep;
    assert!((brep.area(Tolerance::DEFAULT).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    let arc = brep
        .edges()
        .iter()
        .find(|edge| edge.curve().degree() == 2)
        .unwrap();
    for t in [0., 0.25, 0.5, 0.75, 1.] {
        let p = arc.curve().evaluate(t).unwrap();
        assert!(((p.x() / 2.).powi(2) + p.y().powi(2) - 1.).abs() < 1e-12);
    }
}

#[test]
fn native_step_imports_analytic_open_cylinder_patches() {
    use monstertruck::modeling::{
        Invertible, Line, Point2 as TruckPoint2, Processor, RevolutionSurface, Vector3, builder,
    };
    use monstertruck::step::load::step_geometry::{
        Curve2D, Curve3D, ElementarySurface, StepParameterCurve, Surface,
    };
    use monstertruck::step::save::StepModels;
    use monstertruck::topology::compress::{
        CompressedEdgeUse, CompressedTrimmedFace, CompressedTrimmedShell,
    };
    use monstertruck::topology::{Edge, Face, Shell, Vertex, Wire};
    let point = |x, y, z| TruckPoint3::new(x, y, z);
    for angle in [
        std::f64::consts::FRAC_PI_3,
        std::f64::consts::FRAC_PI_2,
        2. * std::f64::consts::FRAC_PI_3,
        5. * std::f64::consts::FRAC_PI_6,
        17. * std::f64::consts::PI / 18.,
        std::f64::consts::PI,
        4. * std::f64::consts::FRAC_PI_3,
        3. * std::f64::consts::FRAC_PI_2,
        35. * std::f64::consts::PI / 18.,
    ] {
        let east_bottom = Vertex::new(point(2., 0., 0.));
        let north_bottom = Vertex::new(point(2. * angle.cos(), 2. * angle.sin(), 0.));
        let north_top = Vertex::new(point(2. * angle.cos(), 2. * angle.sin(), 3.));
        let east_top = Vertex::new(point(2., 0., 3.));
        let diagonal = (2. * (angle / 2.).cos(), 2. * (angle / 2.).sin());
        let edges: Vec<Edge<TruckPoint3, Curve3D>> = vec![
            builder::circle_arc(
                &east_bottom,
                &north_bottom,
                point(diagonal.0, diagonal.1, 0.),
            ),
            builder::line(&north_bottom, &north_top),
            builder::circle_arc(&north_top, &east_top, point(diagonal.0, diagonal.1, 3.)),
            builder::line(&east_top, &east_bottom),
        ];
        let mut cylinder = Processor::new(RevolutionSurface::by_revolution(
            Line(point(2., 0., 0.), point(2., 0., 3.)),
            point(0., 0., 0.),
            Vector3::unit_z(),
        ));
        cylinder.invert();
        let surface = Surface::ElementarySurface(ElementarySurface::CylindricalSurface(cylinder));
        let shell = Shell::from(vec![Face::new(vec![Wire::from(edges)], surface)]).compress();
        let face = &shell.faces[0];
        let uv = [
            ([0., 0.], [angle, 0.]),
            ([angle, 0.], [angle, 3.]),
            ([angle, 3.], [0., 3.]),
            ([0., 3.], [0., 0.]),
        ];
        let boundaries = vec![
            face.boundaries[0]
                .iter()
                .zip(uv)
                .map(|(edge, (start, end))| CompressedEdgeUse {
                    index: edge.index,
                    orientation: edge.orientation,
                    trim_curve: Some(StepParameterCurve::new(
                        Box::new(Curve2D::Line(Line(
                            TruckPoint2::new(start[0], start[1]),
                            TruckPoint2::new(end[0], end[1]),
                        ))),
                        Box::new(face.surface.clone()),
                    )),
                })
                .collect(),
        ];
        let trimmed = CompressedTrimmedShell {
            vertices: shell.vertices,
            edges: shell.edges,
            faces: vec![CompressedTrimmedFace {
                boundaries,
                orientation: face.orientation,
                surface: face.surface.clone(),
            }],
        };
        let mut models = StepModels::default();
        models.push_trimmed_shell(&trimmed);
        let text = CompleteStepDisplay::new(models, StepHeaderDescriptor::default()).to_string();
        assert!(text.contains("CYLINDRICAL_SURFACE("));
        let table = Table::from_step(&text).unwrap();
        assert_eq!(table.entity_report.total(), 0);
        let shell_id = *table.shell.keys().next().unwrap();
        let (decoded, report) = reported_trimmed_shell(&table, shell_id).unwrap();
        assert_eq!(report.total_lost(), 0);
        assert_eq!(decoded.faces[0].boundaries[0].len(), 4);
        assert!(
            decoded.faces[0].boundaries[0]
                .iter()
                .all(|edge| edge.trim_curve.is_some())
        );
        let native = read_step_native_instances(Cursor::new(&text), Tolerance::DEFAULT)
            .unwrap_or_else(|error| panic!("angle {angle}: {error:?}"));
        let brep = &native.instances[0].brep;
        assert_eq!(brep.faces().len(), 1);
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 6. * angle).abs() < 1e-8);
        for u in [0., angle * 0.25, angle * 0.5, angle * 0.75, angle] {
            for v in [0., 1.5, 3.] {
                let point = brep.faces()[0].surface().evaluate(u, v).unwrap();
                assert!(((point.x().hypot(point.y())) - 2.).abs() < 1e-12);
                assert!((point.z() - v).abs() < 1e-12);
            }
        }
        let converted = read_step_native_instances_in_units(
            Cursor::new(&text),
            &LengthUnitSystem::Centimeters,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            (converted.instances[0]
                .brep
                .area(Tolerance::DEFAULT)
                .unwrap()
                - 0.06 * angle)
                .abs()
                < 1e-10
        );
    }
}

#[test]
fn native_step_imports_periodic_cylinder_wall_seam() {
    use monstertruck::modeling::{
        Invertible, Line, Point2 as TruckPoint2, Processor, RevolutionSurface, Transformed,
        TrimmedCurve, UnitCircle, Vector3,
    };
    use monstertruck::step::load::step_geometry::{
        Conic3D, Curve2D, Curve3D, ElementarySurface, StepParameterCurve, Surface,
    };
    use monstertruck::step::save::StepModels;
    use monstertruck::topology::compress::{
        CompressedEdge, CompressedEdgeUse, CompressedTrimmedFace, CompressedTrimmedShell,
    };
    use viboceros_geometry::BrepTrimType;
    let point = |x, y, z| TruckPoint3::new(x, y, z);
    let make_circle = |height| {
        let mut circle = Processor::new(TrimmedCurve::new(
            UnitCircle::<TruckPoint3>::new(),
            (0., std::f64::consts::TAU),
        ));
        circle.transform_by(
            Matrix4::from_translation(Vector3::new(0., 0., height)) * Matrix4::from_scale(2.),
        );
        Curve3D::Conic(Conic3D::Ellipse(circle))
    };
    let mut cylinder = Processor::new(RevolutionSurface::by_revolution(
        Line(point(2., 0., 0.), point(2., 0., 3.)),
        point(0., 0., 0.),
        Vector3::unit_z(),
    ));
    cylinder.invert();
    let surface = Surface::ElementarySurface(ElementarySurface::CylindricalSurface(cylinder));
    for uv_origin in [0., std::f64::consts::TAU] {
        let uv = [
            ([uv_origin, 0.], [uv_origin + std::f64::consts::TAU, 0.]),
            (
                [uv_origin + std::f64::consts::TAU, 0.],
                [uv_origin + std::f64::consts::TAU, 3.],
            ),
            ([uv_origin + std::f64::consts::TAU, 3.], [uv_origin, 3.]),
            ([uv_origin, 0.], [uv_origin, 3.]),
        ];
        let uses = [(0, true), (1, true), (2, false), (1, false)]
            .into_iter()
            .zip(uv)
            .map(|((index, orientation), (start, end))| CompressedEdgeUse {
                index,
                orientation,
                trim_curve: Some(StepParameterCurve::new(
                    Box::new(Curve2D::Line(Line(
                        TruckPoint2::new(start[0], start[1]),
                        TruckPoint2::new(end[0], end[1]),
                    ))),
                    Box::new(surface.clone()),
                )),
            })
            .collect();
        let shell = CompressedTrimmedShell {
            vertices: vec![point(2., 0., 0.), point(2., 0., 3.)],
            edges: vec![
                CompressedEdge {
                    vertices: (0, 0),
                    curve: make_circle(0.),
                },
                CompressedEdge {
                    vertices: (0, 1),
                    curve: Curve3D::Line(Line(point(2., 0., 0.), point(2., 0., 3.))),
                },
                CompressedEdge {
                    vertices: (1, 1),
                    curve: make_circle(3.),
                },
            ],
            faces: vec![CompressedTrimmedFace {
                boundaries: vec![uses],
                orientation: true,
                surface: surface.clone(),
            }],
        };
        let mut models = StepModels::default();
        models.push_trimmed_shell(&shell);
        let text = CompleteStepDisplay::new(models, StepHeaderDescriptor::default()).to_string();
        let mut seam_count = 0;
        let lines = text.lines().collect::<Vec<_>>();
        let text = lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                if line.contains(" = SURFACE_CURVE(")
                    && lines
                        .get(index + 1)
                        .is_some_and(|next| next.contains(" = LINE("))
                {
                    seam_count += 1;
                    line.replace(" = SURFACE_CURVE(", " = SEAM_CURVE(")
                } else {
                    (*line).to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(seam_count, 1);
        let table = Table::from_step(&text).unwrap();
        assert_eq!(table.entity_report.total(), 0);
        let shell_id = *table.shell.keys().next().unwrap();
        let (decoded, report) = reported_trimmed_shell(&table, shell_id).unwrap();
        assert_eq!(report.total_lost(), 0);
        assert_eq!(
            (
                decoded.vertices.len(),
                decoded.edges.len(),
                decoded.faces.len()
            ),
            (2, 3, 1)
        );
        assert!(
            decoded.faces[0].boundaries[0]
                .iter()
                .all(|edge| edge.trim_curve.is_some())
        );
        let native = read_step_native_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
        let brep = &native.instances[0].brep;
        assert_eq!(
            (
                brep.vertices().len(),
                brep.edges().len(),
                brep.faces().len()
            ),
            (2, 3, 1)
        );
        assert_eq!(
            brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| trim.trim_type())
                .collect::<Vec<_>>(),
            vec![
                BrepTrimType::Boundary,
                BrepTrimType::Seam,
                BrepTrimType::Boundary,
                BrepTrimType::Seam,
            ]
        );
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 12. * std::f64::consts::PI).abs() < 1e-8);
    }
}

#[test]
fn native_planar_step_retains_small_holes() {
    for size in [1e-4, 1e-6, 1e-8, 1e-10] {
        let text = polygon_face_step(
            &[
                vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]],
                vec![
                    [4., 4.],
                    [4., 4. + size],
                    [4. + size, 4. + size],
                    [4. + size, 4.],
                ],
            ],
            false,
        );
        let shells = read_step_planar_shells(Cursor::new(text), Tolerance::DEFAULT).unwrap();
        let brep = &shells[0].brep;
        assert_eq!((brep.vertices().len(), brep.edges().len()), (8, 8));
        let loops = brep.faces()[0].loops();
        assert_eq!(loops.len(), 2);
        assert_eq!(loops[1].trims().len(), 4);
    }
}

#[test]
fn planar_step_multiple_holes_convert_and_invalid_regions_fail() {
    let outer = vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]];
    let a = vec![[1., 1.], [1., 3.], [3., 3.], [3., 1.]];
    let b = vec![[5., 5.], [5., 8.], [8., 8.], [8., 5.]];
    let text = polygon_face_step(&[a.clone(), outer.clone(), b], false);
    let native = read_step_planar_shells_in_units(
        Cursor::new(text),
        &LengthUnitSystem::Centimeters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let brep = &native[0].brep;
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces()[0].loops().len()
        ),
        (12, 12, 3)
    );
    assert!((brep.area(Tolerance::DEFAULT).unwrap() - 0.87).abs() < 1e-12);
    for hole in [
        vec![[11., 1.], [11., 2.], [12., 2.], [12., 1.]],
        vec![[0., 1.], [0., 2.], [2., 2.], [2., 1.]],
        vec![[2., 2.], [2., 4.], [4., 4.], [4., 2.]],
        vec![[1.5, 1.5], [1.5, 2.], [2., 2.], [2., 1.5]],
    ] {
        let text = polygon_face_step(&[outer.clone(), a.clone(), hole], false);
        let table = Table::from_step(&text).unwrap();
        let id = *table.shell.keys().next().unwrap();
        assert_eq!(
            reported_trimmed_shell(&table, id).unwrap().1.total_lost(),
            0
        );
        assert!(matches!(
            read_step_planar_shells(Cursor::new(text), Tolerance::DEFAULT),
            Err(StepError::Geometry(
                GeometryError::InvalidPlanarFaceBoundary
            ))
        ));
    }
}

#[test]
fn native_planar_unit_conversion_scales_geometry_but_preserves_uv_trims() {
    let text = cube_step();
    let source = read_step_planar_shells(Cursor::new(&text), Tolerance::DEFAULT).unwrap();
    for (target, scale) in [
        (LengthUnitSystem::Millimeters, 1.0),
        (LengthUnitSystem::Centimeters, 0.1),
        (LengthUnitSystem::Meters, 0.001),
        (LengthUnitSystem::Kilometers, 1e-6),
        (LengthUnitSystem::Microns, 1000.0),
    ] {
        let converted =
            read_step_planar_shells_in_units(Cursor::new(&text), &target, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].source_shell_id, source[0].source_shell_id);
        let actual = &converted[0].brep;
        let original = &source[0].brep;
        assert_eq!(
            (
                actual.vertices().len(),
                actual.edges().len(),
                actual.faces().len()
            ),
            (8, 12, 6)
        );
        for (vertex, old) in actual.vertices().iter().zip(original.vertices()) {
            let expected = old.point().to_array().map(|coordinate| coordinate * scale);
            for (coordinate, expected) in vertex.point().to_array().into_iter().zip(expected) {
                assert!((coordinate - expected).abs() <= expected.abs() * 1e-12);
            }
        }
        for (face, old) in actual.faces().iter().zip(original.faces()) {
            assert_eq!(face.loops(), old.loops());
            assert_eq!(face.is_reversed(), old.is_reversed());
        }
        assert!(
            (actual.area(Tolerance::DEFAULT).unwrap() / (286.0 * scale * scale) - 1.0).abs()
                < 1e-10
        );
        assert!(
            (actual.signed_volume(Tolerance::DEFAULT).unwrap() / (315.0 * scale * scale * scale)
                - 1.0)
                .abs()
                < 1e-10
        );
    }
}

#[test]
fn native_planar_unit_reader_rejects_missing_or_invalid_units_and_preserves_unitless_coordinates() {
    let text = cube_step();
    for target in [
        LengthUnitSystem::Unset,
        LengthUnitSystem::Custom {
            name: "invalid".into(),
            meters_per_unit: f64::NAN,
        },
    ] {
        assert!(matches!(
            read_step_planar_shells_in_units(Cursor::new(&text), &target, Tolerance::DEFAULT),
            Err(StepError::Units(_))
        ));
    }
    assert_eq!(
        read_step_planar_shells_in_units(
            Cursor::new(&text),
            &LengthUnitSystem::None,
            Tolerance::DEFAULT
        )
        .unwrap(),
        read_step_planar_shells(Cursor::new(&text), Tolerance::DEFAULT).unwrap(),
    );
    let missing = text.replace("SI_UNIT(.MILLI.,.METRE.)", "SI_UNIT($,.RADIAN.)");
    assert!(matches!(
        read_step_planar_shells_in_units(
            Cursor::new(missing),
            &LengthUnitSystem::Millimeters,
            Tolerance::DEFAULT
        ),
        Err(StepError::InvalidLengthUnits(_))
    ));
}

#[test]
fn native_planar_reader_preserves_cube_brep_geometry_without_tessellation() {
    let shells = read_step_planar_shells(Cursor::new(cube_step()), Tolerance::DEFAULT).unwrap();
    assert_eq!(shells.len(), 1);
    let brep = &shells[0].brep;
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (8, 12, 6)
    );
    // Plane control rectangles may extend beyond oblique UV trim loops.
    let bounds = viboceros_geometry::BoundingBox3::from_points(
        brep.vertices().iter().map(|vertex| vertex.point()),
    )
    .unwrap();
    assert_eq!(bounds.min(), Point3::try_new(-1.0, -2.0, -3.0).unwrap());
    assert_eq!(bounds.max(), Point3::try_new(4.0, 5.0, 6.0).unwrap());
    assert!((brep.area(Tolerance::DEFAULT).unwrap() - 286.0).abs() < 1e-10);
    assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 315.0).abs() < 1e-10);
    let metres = cube_step().replace("SI_UNIT(.MILLI.,.METRE.)", "SI_UNIT($,.METRE.)");
    assert_eq!(
        read_step_planar_shells(Cursor::new(metres), Tolerance::DEFAULT).unwrap(),
        shells
    );
}

#[test]
fn native_planar_reader_preserves_open_triangle_and_rejects_curved_surfaces() {
    for reversed in [false, true] {
        let mesh = TriangleMesh::try_new(
            vec![
                Point3::try_new(2.0, 3.0, 4.0).unwrap(),
                Point3::try_new(4.0, 3.0, 4.0).unwrap(),
                Point3::try_new(2.0, 6.0, 4.0).unwrap(),
            ],
            vec![if reversed { [0, 2, 1] } else { [0, 1, 2] }],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut bytes = Vec::new();
        write_step(&mut bytes, &[mesh]).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let shells = read_step_planar_shells(Cursor::new(&text), Tolerance::DEFAULT).unwrap();
        assert_eq!(shells.len(), 1);
        let brep = &shells[0].brep;
        assert_eq!(
            (
                brep.vertices().len(),
                brep.edges().len(),
                brep.faces().len()
            ),
            (3, 3, 1)
        );
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 3.0).abs() < 1e-12);
        assert!(brep.signed_volume(Tolerance::DEFAULT).is_err());
        let curved = text
            .lines()
            .map(|line| {
                if line.contains(" = PLANE(") {
                    line.replace(" = PLANE(", " = CYLINDRICAL_SURFACE(")
                        .replace(");", ", 1.0);")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(Table::from_step(&curved).unwrap().entity_report.total(), 0);
        assert!(matches!(
            read_step_planar_shells(Cursor::new(curved), Tolerance::DEFAULT),
            Err(StepError::UnsupportedPlanarShell { .. })
        ));
    }
}

#[test]
fn native_planar_reader_preserves_face_reversal_in_signed_volume() {
    let reversed = cube_step()
        .lines()
        .map(|line| {
            // Reverse the face and its face-relative bound together. Changing
            // same_sense alone would leave an incorrectly oriented outer loop.
            if line.contains(" = ADVANCED_FACE(") || line.contains(" = FACE_BOUND(") {
                if let Some(prefix) = line.strip_suffix(".T.);") {
                    format!("{prefix}.F.);")
                } else if let Some(prefix) = line.strip_suffix(".F.);") {
                    format!("{prefix}.T.);")
                } else {
                    panic!("unexpected face record")
                }
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let shells = read_step_planar_shells(Cursor::new(reversed), Tolerance::DEFAULT).unwrap();
    assert!((shells[0].brep.signed_volume(Tolerance::DEFAULT).unwrap() + 315.0).abs() < 1e-10);
}

#[test]
fn parsed_cube_retains_shared_edges_and_face_local_trim_availability() {
    let table = Table::from_step(&cube_step()).unwrap();
    let shell_id = *table.shell.keys().next().unwrap();
    let (shell, report) = reported_trimmed_shell(&table, shell_id).unwrap();
    assert_eq!(report.total_lost(), 0);
    assert_eq!(
        (shell.vertices.len(), shell.edges.len(), shell.faces.len()),
        (8, 12, 6)
    );
    let mut uses = vec![Vec::new(); shell.edges.len()];
    let mut trim_count = 0;
    for face in &shell.faces {
        assert_eq!(face.boundaries.len(), 1);
        assert_eq!(face.boundaries[0].len(), 4);
        for (side, edge_use) in face.boundaries[0].iter().enumerate() {
            uses[edge_use.index].push(edge_use.orientation);
            trim_count += usize::from(edge_use.trim_curve.is_some());
            let edge = &shell.edges[edge_use.index];
            let trim = edge_use
                .trim_curve
                .as_ref()
                .expect("cube face has an exact UV trim");
            let (trim_start, trim_end) = trim.range_tuple();
            let directed = if edge_use.orientation {
                edge.vertices
            } else {
                (edge.vertices.1, edge.vertices.0)
            };
            for station in 0..=4 {
                let fraction = f64::from(station) / 4.0;
                let parameter = trim_start + (trim_end - trim_start) * fraction;
                let uv = trim.curve().evaluate(parameter);
                let actual = face.surface.evaluate(uv.x, uv.y);
                let expected = shell.vertices[directed.0]
                    + (shell.vertices[directed.1] - shell.vertices[directed.0]) * fraction;
                let error = actual - expected;
                assert!(error.x.hypot(error.y).hypot(error.z) < 1e-12);
            }
            let next_use = &face.boundaries[0][(side + 1) % 4];
            let next = &shell.edges[next_use.index];
            let end = if edge_use.orientation {
                edge.vertices.1
            } else {
                edge.vertices.0
            };
            let start = if next_use.orientation {
                next.vertices.0
            } else {
                next.vertices.1
            };
            assert_eq!(end, start);
        }
    }
    for (edge, orientations) in shell.edges.iter().zip(uses) {
        assert_eq!(orientations.len(), 2);
        assert_ne!(orientations[0], orientations[1]);
        let (start, end) = edge.curve.range_tuple();
        assert_eq!(edge.curve.evaluate(start), shell.vertices[edge.vertices.0]);
        assert_eq!(edge.curve.evaluate(end), shell.vertices[edge.vertices.1]);
    }
    assert_eq!(trim_count, 24);
}

#[test]
fn unit_aware_export_accuracy_is_parseable_and_exact_across_finite_scales() {
    let mesh = unit_test_mesh();
    for accuracy in [f64::MIN_POSITIVE, 1e-100, 1e-6, 1.0, 1e21, 1e100, f64::MAX] {
        let tolerance = Tolerance::try_new(accuracy, 1e-10, 1e-10).unwrap();
        let mut output = Vec::new();
        write_step_in_units(
            &mut output,
            std::slice::from_ref(&mesh),
            &LengthUnitSystem::Millimeters,
            tolerance,
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        // Parse the complete file, not just the real literal as a Rust float.
        let table = Table::from_step(&text).unwrap();
        assert_eq!(table.entity_report.total(), 0);
        let literal = text
            .split("LENGTH_MEASURE(")
            .nth(1)
            .unwrap()
            .split(')')
            .next()
            .unwrap();
        assert_eq!(
            literal.parse::<f64>().unwrap().to_bits(),
            accuracy.to_bits()
        );
        assert!(literal.split('E').next().unwrap().contains('.'));
    }
}

#[test]
fn step_real_format_preserves_finite_binary64_values_and_required_decimal_point() {
    use super::export_geometry::StepReal;
    for exponent in 0..2047_u64 {
        for fraction in [0, 1, (1_u64 << 52) - 1] {
            for sign in [0, 1_u64 << 63] {
                let value = f64::from_bits(sign | (exponent << 52) | fraction);
                let text = StepReal(value).to_string();
                let (mantissa, _) = text.split_once('E').unwrap();
                assert!(mantissa.contains('.'));
                assert_eq!(text.parse::<f64>().unwrap().to_bits(), value.to_bits());
            }
        }
    }
}

#[test]
fn exported_plane_directions_remain_unit_length_across_mesh_scales() {
    for scale in [1e-200, 1e-100, 1e-20, 1.0, 1e20, 1e100, 1e200] {
        for flipped in [false, true] {
            let mesh = TriangleMesh::try_new(
                vec![
                    Point3::try_new(10.0 * scale, 20.0 * scale, 30.0 * scale).unwrap(),
                    Point3::try_new(11.0 * scale, 20.0 * scale, 30.0 * scale).unwrap(),
                    Point3::try_new(10.0 * scale, 21.0 * scale, 30.0 * scale).unwrap(),
                ],
                vec![if flipped { [0, 2, 1] } else { [0, 1, 2] }],
                Tolerance::MESH_VALIDATION,
            )
            .unwrap();
            let mut output = Vec::new();
            write_step(&mut output, &[mesh]).unwrap();
            let text = String::from_utf8(output).unwrap();
            let table = Table::from_step(&text).unwrap_or_else(|error| panic!("{error}\n{text}"));
            assert_eq!(table.entity_report.total(), 0);
            assert_eq!(table.plane.len(), 1);
            assert_eq!(table.direction.len(), 5);
            let mut lengths = table
                .vector
                .values()
                .map(|vector| vector.magnitude)
                .collect::<Vec<_>>();
            lengths.sort_by(f64::total_cmp);
            assert_eq!(lengths.len(), 3);
            for (actual, expected) in
                lengths
                    .into_iter()
                    .zip([scale, scale, scale * 2.0_f64.sqrt()])
            {
                assert!(actual.is_finite() && actual > 0.0);
                assert!((actual / expected - 1.0).abs() < 1e-12);
            }
            for direction in table.direction.values() {
                let ratios = &direction.direction_ratios;
                assert_eq!(ratios.len(), 3);
                assert!(ratios.iter().all(|value| value.is_finite()));
                let length = ratios[0].hypot(ratios[1]).hypot(ratios[2]);
                assert!(
                    (length - 1.0).abs() < 1e-12,
                    "scale={scale}, direction={ratios:?}"
                );
            }
            let normal = vec![0.0, 0.0, if flipped { -1.0 } else { 1.0 }];
            assert!(
                table
                    .direction
                    .values()
                    .any(|direction| direction.direction_ratios == normal)
            );
        }
    }
}

#[test]
fn unrepresentable_export_directions_preserve_stream_and_destination() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.step");
    std::fs::write(&path, b"original").unwrap();
    for scale in [f64::MAX, f64::MAX * 0.9] {
        let mesh = TriangleMesh::try_new(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(scale, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, scale, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::MESH_VALIDATION,
        )
        .unwrap();
        // A valid earlier mesh must not cause partial output before a later
        // mesh fails its serialization preflight.
        let meshes = [unit_test_mesh(), mesh];
        let mut stream = b"original".to_vec();
        assert!(matches!(
            write_step(&mut stream, &meshes),
            Err(StepError::InvalidExportDirections { face: 0 })
        ));
        assert_eq!(stream, b"original");
        assert!(matches!(
            write_step_file(&path, &meshes),
            Err(StepError::InvalidExportDirections { face: 0 })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }
}

#[test]
fn tessellation_sampling_preserves_finite_extreme_parameter_ranges() {
    for (start, end) in [
        (-f64::MAX, f64::MAX),
        (f64::MAX, -f64::MAX),
        (f64::MAX / 2.0, f64::MAX),
        (-f64::MAX, -f64::MAX / 2.0),
        (0.0, 1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
    ] {
        let samples =
            std::array::from_fn::<_, 5, _>(|index| sample_parameter(start, end, index as u32));
        assert_eq!(samples[0], start);
        assert_eq!(samples[4], end);
        assert!(samples.iter().all(|value| value.is_finite()
            && *value >= start.min(end)
            && *value <= start.max(end)));
        assert!(samples.windows(2).all(|pair| if start <= end {
            pair[0] <= pair[1]
        } else {
            pair[0] >= pair[1]
        }));
    }
    assert_eq!(sample_parameter(-f64::MAX, f64::MAX, 2), 0.0);
    assert_eq!(sample_parameter(-f64::MAX, f64::MAX, 1), -f64::MAX / 2.0);
}

#[test]
fn relative_tessellation_extent_scales_before_overflow() {
    assert_eq!(SampledExtent::default().relative_diameter(), 0.0);
    for scale in [1.0, f64::MAX / 2.0, f64::MAX] {
        let mut extent = SampledExtent::default();
        extent
            .push(TruckPoint3::new(-scale, -scale, -scale))
            .unwrap();
        extent.push(TruckPoint3::new(scale, scale, scale)).unwrap();
        let expected = (scale * RELATIVE_MESH_TOLERANCE) * (2.0 * 3.0_f64.sqrt());
        let actual = extent.relative_diameter();
        assert!(actual.is_finite() && actual > 0.0);
        assert!((actual / expected - 1.0).abs() < 1e-14);
    }
    let mut singleton = SampledExtent::default();
    singleton
        .push(TruckPoint3::new(f64::MAX, f64::MAX, f64::MAX))
        .unwrap();
    assert_eq!(singleton.relative_diameter(), 0.0);
}

#[test]
fn sampled_extent_rejects_nonfinite_coordinates_without_partial_updates() {
    for axis in 0..3 {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut extent = SampledExtent::default();
            extent.push(TruckPoint3::new(1.0, 2.0, 3.0)).unwrap();
            let mut coordinates = [-99.0; 3];
            coordinates[axis] = value;
            assert!(
                extent
                    .push(TruckPoint3::new(
                        coordinates[0],
                        coordinates[1],
                        coordinates[2]
                    ))
                    .is_err()
            );
            assert_eq!(extent.minimum, [1.0, 2.0, 3.0]);
            assert_eq!(extent.maximum, [1.0, 2.0, 3.0]);
            assert_eq!(extent.count, 1);
        }
    }
}

#[test]
fn tessellation_setup_propagates_invalid_shell_samples() {
    let table = Table::from_step(&cube_step()).unwrap();
    let id = *table.shell.keys().next().unwrap();
    let (mut shell, _) = reported_trimmed_shell(&table, id).unwrap();
    assert!(
        tessellation_tolerance(std::iter::once(&shell), Tolerance::DEFAULT)
            .unwrap()
            .is_finite()
    );
    shell.vertices[0].z = f64::NAN;
    assert!(matches!(
        tessellation_tolerance(std::iter::once(&shell), Tolerance::DEFAULT),
        Err(StepError::Geometry(_))
    ));
}

#[test]
fn unsupported_data_section_counts_return_errors_in_both_readers() {
    let cube = cube_step();
    let empty = format!("{}END-ISO-10303-21;", cube.split("DATA;").next().unwrap());
    let multiple = cube.replace("END-ISO-10303-21;", "DATA;\nENDSEC;\nEND-ISO-10303-21;");
    for (text, count) in [(empty, 0), (multiple, 2)] {
        let parsed = monstertruck::step::load::step_p21::parser::parse(&text).unwrap();
        assert_eq!(parsed.data.len(), count);
        assert!(
            matches!(read_step(Cursor::new(&text), Tolerance::DEFAULT), Err(StepError::UnsupportedDataSections { count: actual }) if actual == count)
        );
        assert!(matches!(
            read_step_in_units(
                Cursor::new(&text),
                &LengthUnitSystem::Millimeters,
                Tolerance::DEFAULT
            ), Err(StepError::UnsupportedDataSections { count: actual }) if actual == count));
    }
}

#[test]
fn both_step_readers_preserve_geometry_with_legacy_header_bytes() {
    let mut bytes = cube_step().into_bytes();
    let marker = b"Viboceros test";
    let index = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap();
    bytes[index] = 0xe9;
    assert!(std::str::from_utf8(&bytes).is_err());
    for imported in [
        read_step(Cursor::new(&bytes), Tolerance::DEFAULT).unwrap(),
        read_step_in_units(
            Cursor::new(&bytes),
            &LengthUnitSystem::Millimeters,
            Tolerance::DEFAULT,
        )
        .unwrap(),
    ] {
        assert_eq!(imported.objects.len(), 1);
        assert_eq!(imported.objects[0].mesh.triangles().len(), 12);
        let bounds = imported.objects[0].mesh.bounds();
        assert!(bounds.min().is_near(
            Point3::try_new(-1.0, -2.0, -3.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert!(
            bounds
                .max()
                .is_near(Point3::try_new(4.0, 5.0, 6.0).unwrap(), Tolerance::DEFAULT)
        );
    }
}

fn unit_test_mesh() -> TriangleMesh {
    TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn unit_aware_step_export_matches_its_millimetre_declaration() {
    let mesh = unit_test_mesh();
    let original = mesh.clone();
    for (units, factor) in [
        (LengthUnitSystem::Millimeters, 1.0),
        (LengthUnitSystem::Meters, 1000.0),
        (LengthUnitSystem::Inches, 25.4),
        (
            LengthUnitSystem::Custom {
                name: "eighth-metre".into(),
                meters_per_unit: 0.125,
            },
            125.0,
        ),
    ] {
        let mut bytes = Vec::new();
        write_step_in_units(
            &mut bytes,
            std::slice::from_ref(&mesh),
            &units,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("SI_UNIT(.MILLI.,.METRE.)"));
        let imported = read_step(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.objects.len(), 1);
        let mesh = &imported.objects[0].mesh;
        assert_eq!(mesh.triangles().len(), 1);
        assert!(
            mesh.bounds()
                .min()
                .is_near(Point3::try_new(0.0, 0.0, 0.0).unwrap(), Tolerance::DEFAULT)
        );
        assert!(mesh.bounds().max().is_near(
            Point3::try_new(factor, factor * 2.0, 0.0).unwrap(),
            Tolerance::DEFAULT
        ));
    }
    assert_eq!(mesh, original);
}

#[test]
fn invalid_export_units_preserve_the_destination_and_stream() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.step");
    std::fs::write(&path, b"original").unwrap();
    let mesh = unit_test_mesh();
    for units in [
        LengthUnitSystem::None,
        LengthUnitSystem::Unset,
        LengthUnitSystem::Custom {
            name: "invalid".into(),
            meters_per_unit: f64::NAN,
        },
        LengthUnitSystem::Custom {
            name: "overflow".into(),
            meters_per_unit: f64::MAX,
        },
    ] {
        assert!(
            write_step_file_in_units(
                &path,
                std::slice::from_ref(&mesh),
                &units,
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        let mut stream = b"original".to_vec();
        assert!(
            write_step_in_units(
                &mut stream,
                std::slice::from_ref(&mesh),
                &units,
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert_eq!(stream, b"original");
    }
}

#[test]
fn unit_aware_export_scales_validation_and_declared_accuracy() {
    let mesh = unit_test_mesh()
        .transformed(
            AffineTransform3::try_uniform_scale(Point3::try_new(0.0, 0.0, 0.0).unwrap(), 1e-5)
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
    let mut bytes = Vec::new();
    write_step_in_units(
        &mut bytes,
        &[mesh],
        &LengthUnitSystem::Nanometers,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let table = Table::from_step(text).unwrap();
    assert!(table.cartesian_point.values().any(|point| {
        point
            .coordinates
            .first()
            .is_some_and(|x| (*x - 1e-11).abs() < 1e-26)
    }));
    // The uncertainty value is in the same millimetres as the coordinates.
    let accuracy = text
        .split("LENGTH_MEASURE(")
        .nth(1)
        .unwrap()
        .split(')')
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((accuracy - 1e-15).abs() < 1e-30);
}
use monstertruck::modeling::{BoundingBox, Point3 as TruckPoint3, primitive};
use monstertruck::step::save::{
    CompleteStepDisplay, StepHeaderDescriptor, StepModel as TruckStepModel,
};

fn cube_step() -> String {
    let cube: monstertruck::modeling::Solid = primitive::cuboid(BoundingBox::from_iter([
        TruckPoint3::new(-1.0, -2.0, -3.0),
        TruckPoint3::new(4.0, 5.0, 6.0),
    ]));
    let compressed = cube.compress();
    CompleteStepDisplay::new(
        TruckStepModel::from(&compressed),
        StepHeaderDescriptor {
            organization_system: "Viboceros test".to_owned(),
            ..Default::default()
        },
    )
    .to_string()
}

#[test]
fn planar_brep_step_export_keeps_cube_faces_edges_and_volume_editable() {
    let source = read_step_planar_instances(Cursor::new(cube_step()), Tolerance::DEFAULT).unwrap();
    assert_eq!(source.instances.len(), 1);
    let brep = &source.instances[0].brep;
    let mut output = Vec::new();
    write_step_planar_breps(&mut output, std::slice::from_ref(brep)).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert_eq!(text.matches("ADVANCED_FACE(").count(), brep.faces().len());
    assert_eq!(text.matches("EDGE_CURVE(").count(), brep.edges().len());
    assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
    assert!(!text.contains("TRIANGULATED_FACE_SET"));
    let table = Table::from_step(&text).unwrap();
    assert_eq!(table.entity_report.total(), 0);
    let restored = read_step_planar_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(restored.instances.len(), 1);
    let actual = &restored.instances[0].brep;
    assert_eq!(actual.vertices().len(), brep.vertices().len());
    assert_eq!(actual.edges().len(), brep.edges().len());
    assert_eq!(actual.faces().len(), brep.faces().len());
    assert!(
        (actual.signed_volume(Tolerance::DEFAULT).unwrap()
            - brep.signed_volume(Tolerance::DEFAULT).unwrap())
        .abs()
            < 1e-9
    );
}

#[test]
fn planar_brep_step_export_certifies_tetrahedron_and_sheared_box() {
    use viboceros_geometry::{AffineTransform3, Brep, Frame3, TriangleMesh, Vector3};
    let tolerance = Tolerance::DEFAULT;
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let tetra = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(0., 4., 0.),
            point(0., 0., 4.),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        tolerance,
    )
    .unwrap();
    let tetra = Brep::try_from_mesh(&tetra, true, tolerance).unwrap();
    let frame = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        tolerance,
    )
    .unwrap();
    let box_brep = Brep::try_box(frame, [[0., 2.]; 3], tolerance).unwrap();
    let shear = AffineTransform3::try_shear(
        point(0., 0., 0.),
        frame.y_axis(),
        frame.x_axis(),
        0.5,
        tolerance,
    )
    .unwrap();
    let sheared = box_brep.transformed(shear, tolerance).unwrap();
    for source in [&tetra, &sheared] {
        assert_eq!(source.certified_convex_solid_shell_order(), Some(vec![0]));
        let mut output = Vec::new();
        write_step_planar_breps(&mut output, [source]).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
        let restored = read_step_planar_instances(Cursor::new(text), tolerance).unwrap();
        assert_eq!(restored.instances.len(), 1);
        assert_eq!(
            restored.instances[0].brep.faces().len(),
            source.faces().len()
        );
        assert!(
            (restored.instances[0].brep.signed_volume(tolerance).unwrap()
                - source.signed_volume(tolerance).unwrap())
            .abs()
                < 1e-9
        );
    }
    let outer = Brep::try_box(frame, [[-5., 5.]; 3], tolerance).unwrap();
    for cavity in [&tetra, &sheared] {
        let source =
            Brep::try_combine(vec![cavity.clone().reversed(), outer.clone()], tolerance).unwrap();
        assert_eq!(
            source.certified_convex_solid_shell_order(),
            Some(vec![1, 0])
        );
        let mut output = Vec::new();
        write_step_planar_breps(&mut output, [&source]).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.matches("BREP_WITH_VOIDS(").count(), 1);
        assert_eq!(text.matches("ORIENTED_CLOSED_SHELL(").count(), 1);
        let restored = read_step_planar_instances(Cursor::new(text), tolerance).unwrap();
        assert_eq!(restored.instances.len(), 2);
        assert_eq!(
            restored.instances[0].source_shape_id,
            restored.instances[1].source_shape_id
        );
    }
    let large_outer = Brep::try_box(frame, [[-10., 10.]; 3], tolerance).unwrap();
    for (offset, separated) in [(1., false), (2., false), (4., true)] {
        let neighbor = sheared
            .transformed(
                AffineTransform3::from_translation(Vector3::try_new(offset, 0., 0.).unwrap()),
                tolerance,
            )
            .unwrap();
        let source = Brep::try_combine(
            vec![
                sheared.clone().reversed(),
                neighbor.reversed(),
                large_outer.clone(),
            ],
            tolerance,
        )
        .unwrap();
        assert_eq!(
            source.certified_convex_solid_shell_order(),
            separated.then_some(vec![2, 0, 1])
        );
        let mut output = Vec::new();
        write_step_planar_breps(&mut output, [&source]).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(
            text.matches("SHELL_BASED_SURFACE_MODEL(").count(),
            if separated { 0 } else { 3 }
        );
        assert_eq!(text.contains("BREP_WITH_VOIDS("), separated);
    }
}

#[test]
fn planar_brep_step_export_retains_certified_box_cavity_as_one_solid() {
    use viboceros_geometry::{Brep, Frame3, Vector3};
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let outer = Brep::try_box(frame, [[-5., 5.]; 3], Tolerance::DEFAULT).unwrap();
    let cavity = Brep::try_box(frame, [[-1., 1.]; 3], Tolerance::DEFAULT)
        .unwrap()
        .reversed();
    // Deliberately put the void first. STEP still requires the outer shell first.
    let source = Brep::try_combine(vec![cavity, outer], Tolerance::DEFAULT).unwrap();
    assert!((source.signed_volume(Tolerance::DEFAULT).unwrap() - 992.).abs() < 1e-9);
    let mut output = Vec::new();
    write_step_planar_breps(&mut output, [&source]).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert_eq!(text.matches("BREP_WITH_VOIDS(").count(), 1);
    assert_eq!(text.matches("ORIENTED_CLOSED_SHELL(").count(), 1);
    let restored = read_step_planar_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(restored.instances.len(), 2);
    assert_eq!(
        restored.instances[0].placement_index,
        restored.instances[1].placement_index
    );
    assert_eq!(
        restored.instances[0].source_shape_id,
        restored.instances[1].source_shape_id
    );
    let combined = Brep::try_combine(
        restored
            .instances
            .into_iter()
            .map(|instance| instance.brep)
            .collect(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!((combined.faces().len(), combined.edges().len()), (12, 24));
    assert!((combined.signed_volume(Tolerance::DEFAULT).unwrap() - 992.).abs() < 1e-9);
}

#[test]
fn planar_brep_step_export_does_not_label_disjoint_boxes_as_voids() {
    use viboceros_geometry::{Brep, Frame3, Vector3};
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let left = Brep::try_box(frame, [[0., 1.], [0., 1.], [0., 1.]], Tolerance::DEFAULT).unwrap();
    let right = Brep::try_box(frame, [[3., 4.], [0., 1.], [0., 1.]], Tolerance::DEFAULT).unwrap();
    let source = Brep::try_combine(vec![left, right], Tolerance::DEFAULT).unwrap();
    let mut output = Vec::new();
    write_step_planar_breps(&mut output, [&source]).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("BREP_WITH_VOIDS("));
    assert_eq!(text.matches("SHELL_BASED_SURFACE_MODEL(").count(), 2);
    let restored = read_step_planar_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(restored.instances.len(), 2);
    assert_ne!(
        restored.instances[0].source_shape_id,
        restored.instances[1].source_shape_id
    );
}

#[test]
fn planar_brep_step_export_requires_separated_contained_void_boxes() {
    use viboceros_geometry::{Brep, Frame3, Vector3};
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let make_box =
        |x: [f64; 2]| Brep::try_box(frame, [x, [-1., 1.], [-1., 1.]], Tolerance::DEFAULT).unwrap();
    let outer = Brep::try_box(frame, [[-10., 10.]; 3], Tolerance::DEFAULT).unwrap();
    for (second, expected_voids) in [([2., 4.], 2), ([-2., 0.], 0), ([-11., -9.], 0)] {
        let source = Brep::try_combine(
            vec![
                make_box([-4., -2.]).reversed(),
                make_box(second).reversed(),
                outer.clone(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut output = Vec::new();
        write_step_planar_breps(&mut output, [&source]).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(
            text.matches("ORIENTED_CLOSED_SHELL(").count(),
            expected_voids
        );
        assert_eq!(text.contains("BREP_WITH_VOIDS("), expected_voids != 0);
    }
}

#[test]
fn planar_brep_step_export_keeps_polygon_hole_and_rejects_curved_edges_atomically() {
    use viboceros_geometry::{Brep, Frame3, Vector3};
    let source = polygon_face_step(
        &[
            vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]],
            vec![[2., 2.], [2., 4.], [4., 4.], [4., 2.]],
        ],
        false,
    );
    let imported = read_step_planar_instances(Cursor::new(source), Tolerance::DEFAULT).unwrap();
    let brep = &imported.instances[0].brep;
    let mut output = Vec::new();
    write_step_planar_breps(&mut output, std::slice::from_ref(brep)).unwrap();
    let restored = read_step_planar_instances(Cursor::new(output), Tolerance::DEFAULT).unwrap();
    assert_eq!(restored.instances.len(), 1);
    assert_eq!(restored.instances[0].brep.faces()[0].loops().len(), 2);
    assert!((restored.instances[0].brep.area(Tolerance::DEFAULT).unwrap() - 96.).abs() < 1e-9);

    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cylinder = Brep::try_cylinder(frame, 2., 0., 3., Tolerance::DEFAULT).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.step");
    std::fs::write(&path, b"original").unwrap();
    assert!(matches!(
        write_step_planar_breps_file(&path, &[brep.clone(), cylinder]),
        Err(StepError::UnsupportedNativeBrep { brep: 1, .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
}

#[test]
fn imports_si_length_units_into_target_coordinates() {
    let metres = cube_step().replace("SI_UNIT(.MILLI.,.METRE.)", "SI_UNIT($,.METRE.)");
    let imported = read_step_in_units(
        Cursor::new(metres),
        &LengthUnitSystem::Millimeters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(imported.objects.len(), 1);
    let bounds = imported.objects[0].mesh.bounds();
    assert!(bounds.min().is_near(
        Point3::try_new(-1000.0, -2000.0, -3000.0).unwrap(),
        Tolerance::DEFAULT
    ));
    assert!(bounds.max().is_near(
        Point3::try_new(4000.0, 5000.0, 6000.0).unwrap(),
        Tolerance::DEFAULT
    ));
    let imported = read_step_in_units(
        Cursor::new(cube_step()),
        &LengthUnitSystem::Centimeters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(
        imported.objects[0]
            .mesh
            .bounds()
            .max()
            .is_near(Point3::try_new(0.4, 0.5, 0.6).unwrap(), Tolerance::DEFAULT)
    );
}

#[test]
fn step_unit_conversion_retains_meshes_smaller_than_model_tolerance() {
    let tolerance = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let imported = read_step_in_units(
        Cursor::new(cube_step()),
        &LengthUnitSystem::Kilometers,
        tolerance,
    )
    .unwrap();
    assert_eq!(imported.objects.len(), 1);
    let mesh = &imported.objects[0].mesh;
    let check = Tolerance::try_new(1e-18, 1e-12, 1e-10).unwrap();
    assert!(
        mesh.bounds()
            .min()
            .is_near(Point3::try_new(-1e-6, -2e-6, -3e-6).unwrap(), check)
    );
    assert!(
        mesh.bounds()
            .max()
            .is_near(Point3::try_new(4e-6, 5e-6, 6e-6).unwrap(), check)
    );
    assert_eq!(mesh.topology().boundary_edge_count(), 0);
    let mut exported = Vec::new();
    write_step_in_units(
        &mut exported,
        std::slice::from_ref(mesh),
        &LengthUnitSystem::Kilometers,
        tolerance,
    )
    .unwrap();
    let restored = read_step(Cursor::new(exported), Tolerance::DEFAULT).unwrap();
    // The imported cube has raw seams between its six meshed faces. Export
    // preserves those seams as six edge-connected shells, not one disconnected
    // shell. Reassemble only for the aggregate geometry/units assertions.
    assert_eq!(restored.objects.len(), 6);
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for object in &restored.objects {
        assert_eq!(object.mesh.triangles().len(), 2);
        assert_eq!(object.mesh.topology().boundary_edge_count(), 4);
        let offset = vertices.len() as u32;
        vertices.extend_from_slice(object.mesh.vertices());
        triangles.extend(
            object
                .mesh
                .triangles()
                .iter()
                .map(|face| face.map(|raw| raw + offset)),
        );
    }
    let mesh = TriangleMesh::try_new(vertices, triangles, Tolerance::MESH_VALIDATION).unwrap();
    assert_eq!(mesh.area().unwrap(), 286.0);
    assert!(mesh.bounds().min().is_near(
        Point3::try_new(-1.0, -2.0, -3.0).unwrap(),
        Tolerance::DEFAULT
    ));
    assert!(
        mesh.bounds()
            .max()
            .is_near(Point3::try_new(4.0, 5.0, 6.0).unwrap(), Tolerance::DEFAULT)
    );
    assert_eq!(mesh.topology().boundary_edge_count(), 0);
}

#[test]
fn step_unit_conversion_rejects_collapsed_triangles_before_writing() {
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1e-100, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1e-100, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    let mut stream = b"existing".to_vec();
    let result = write_step_in_units(
        &mut stream,
        &[mesh],
        &LengthUnitSystem::Custom {
            name: "tiny".into(),
            meters_per_unit: 1e-250,
        },
        Tolerance::DEFAULT,
    );
    assert!(matches!(
        result,
        Err(StepError::Geometry(
            GeometryError::DegenerateTriangle { .. }
        ))
    ));
    assert_eq!(stream, b"existing");
}

#[test]
fn imports_conversion_based_length_units_by_factor_not_name() {
    let text = cube_step().replace(
        "#12 = ( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) );",
        "#12 = ( CONVERSION_BASED_UNIT('arbitrary name',#900001) LENGTH_UNIT() NAMED_UNIT(#900002) );\n#900001 = LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(0.0254),#900003);\n#900002 = DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);\n#900003 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.));",
    );
    assert!(text.contains("arbitrary name"));
    let imported = read_step_in_units(
        Cursor::new(text),
        &LengthUnitSystem::Millimeters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(imported.objects.len(), 1);
    assert!(imported.objects[0].mesh.bounds().min().is_near(
        Point3::try_new(-25.4, -50.8, -76.2).unwrap(),
        Tolerance::DEFAULT
    ));
    assert!(imported.objects[0].mesh.bounds().max().is_near(
        Point3::try_new(101.6, 127.0, 152.4).unwrap(),
        Tolerance::DEFAULT
    ));
}

#[test]
fn rejects_missing_mixed_and_cyclic_step_units() {
    let cube = cube_step();
    for text in [
        cube.replace("GLOBAL_UNIT_ASSIGNED_CONTEXT", "UNSUPPORTED_UNIT_CONTEXT"),
        cube.replace("NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.)", "NAMED_UNIT(#900002) CONVERSION_BASED_UNIT('cycle',#900001)").replace("#13 =", "#900001 = LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.),#12);\n#900002 = DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);\n#13 ="),
        cube.replace("#13 =", "#900001 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.));\n#900002 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#900001));\n#13 ="),
        cube.replace("SI_UNIT(.MILLI.,.METRE.)", "SI_UNIT(.UNKNOWN.,.METRE.)"),
        cube.replace("SI_UNIT($,.RADIAN.)", "SI_UNIT(.MILLI.,.RADIAN.)"),
        cube.replace("GLOBAL_UNIT_ASSIGNED_CONTEXT((#12, #13, #14))", "GLOBAL_UNIT_ASSIGNED_CONTEXT((#12, #12, #13, #14))"),
    ] {
        assert_ne!(text, cube, "fixture mutation did not apply");
        assert!(matches!(read_step_in_units(Cursor::new(text), &LengthUnitSystem::Millimeters, Tolerance::DEFAULT), Err(StepError::InvalidLengthUnits(_))));
    }
}

#[test]
fn imports_an_analytic_step_solid_as_a_validated_mesh() {
    let model = read_step(Cursor::new(cube_step()), Tolerance::DEFAULT).unwrap();

    assert_eq!(model.objects.len(), 1);
    let mesh = &model.objects[0].mesh;
    assert_eq!(mesh.triangles().len(), 12);
    assert!(mesh.bounds().min().is_near(
        Point3::try_new(-1.0, -2.0, -3.0).unwrap(),
        Tolerance::DEFAULT
    ));
    assert!(
        mesh.bounds()
            .max()
            .is_near(Point3::try_new(4.0, 5.0, 6.0).unwrap(), Tolerance::DEFAULT)
    );
    assert_eq!(model.report.swallowed_entity_count, 0);
    assert_eq!(model.report.lost_topology_item_count, 0);
}

fn assembly_step(parent_transform: Option<Matrix4>) -> String {
    use monstertruck::assembly::assy::{Assembly, EdgeEntity, NodeEntity};
    use monstertruck::modeling::Vector3;
    use monstertruck::step::{common::PartAttributes, save::StepDesign};

    // Adapt the standalone writer's data section into an indexed shape.
    // The assembly emitter requires the solid entity at its first index.
    struct IndexedSolid {
        data: String,
        solid: usize,
        length: usize,
    }
    impl monstertruck::step::save::StepLength for IndexedSolid {
        fn step_length(&self) -> usize {
            self.length
        }
    }
    impl monstertruck::step::save::StepFormat for IndexedSolid {
        fn fmt(&self, idx: usize, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut pieces = self.data.split('#');
            write!(f, "{}", pieces.next().unwrap())?;
            for piece in pieces {
                let digits = piece.bytes().take_while(u8::is_ascii_digit).count();
                let original: usize = piece[..digits].parse().unwrap();
                let mapped = if original == self.solid { 0 } else { original };
                write!(f, "#{}{}", idx + mapped, &piece[digits..])?;
            }
            Ok(())
        }
    }
    let cube = cube_step();
    let table = Table::from_step(&cube).unwrap();
    let solid = *table.manifold_solid_brep.keys().next().unwrap() as usize;
    let data = cube
        .split("DATA;")
        .nth(1)
        .unwrap()
        .split("ENDSEC;")
        .next()
        .unwrap()
        .to_owned();
    // Discard the standalone product/context: only the geometry belongs
    // in the new assembly, otherwise it creates an extra root instance.
    let data = data[data.find(&format!("#{solid} =")).unwrap()..].to_owned();
    let length = data
        .split('#')
        .skip(1)
        .map(|piece| {
            let digits = piece.bytes().take_while(u8::is_ascii_digit).count();
            piece[..digits].parse::<usize>().unwrap()
        })
        .max()
        .unwrap()
        + 1;
    let mut assembly = Assembly::new();
    let root = assembly.create_node(NodeEntity {
        shape: None,
        attrs: PartAttributes::default(),
    });
    let parent = if let Some(matrix) = parent_transform {
        let parent = assembly.create_node(NodeEntity {
            shape: None,
            attrs: PartAttributes::default(),
        });
        assembly.create_edge(
            root,
            parent,
            EdgeEntity {
                matrix,
                attrs: PartAttributes {
                    name: "parent".to_owned(),
                    ..Default::default()
                },
            },
        );
        parent
    } else {
        root
    };
    let part = assembly.create_node(NodeEntity {
        shape: Some(IndexedSolid {
            data,
            solid,
            length,
        }),
        attrs: PartAttributes::default(),
    });
    for index in 0..3 {
        assembly.create_edge(
            parent,
            part,
            EdgeEntity {
                matrix: Matrix4::from_translation(Vector3::new(10.0 * index as f64, 0.0, 0.0)),
                attrs: PartAttributes {
                    name: format!("instance {index}"),
                    ..Default::default()
                },
            },
        );
    }
    CompleteStepDisplay::new(StepDesign::new(assembly), StepHeaderDescriptor::default()).to_string()
}

#[test]
fn instance_plan_order_is_stable_across_parses_with_duplicate_names() {
    let original = assembly_step(None);
    let duplicate_names = original
        .replace("instance 0", "same name")
        .replace("instance 1", "same name")
        .replace("instance 2", "same name");
    for text in [original, duplicate_names] {
        let expected = instance_plan::build(&Table::from_step(&text).unwrap()).unwrap();
        assert_eq!(expected.instances.len(), 3);
        for _ in 0..32 {
            let actual = instance_plan::build(&Table::from_step(&text).unwrap()).unwrap();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn native_instance_units_scale_nested_placements_and_preserve_uv_and_metadata() {
    let parent = Matrix4::new(
        0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 0., 100., 200., 300., 1.,
    );
    let text = assembly_step(Some(parent));
    let source = read_step_planar_instances(Cursor::new(&text), Tolerance::DEFAULT).unwrap();
    for (target, scale) in [
        (LengthUnitSystem::Millimeters, 1.),
        (LengthUnitSystem::Centimeters, 0.1),
        (LengthUnitSystem::Meters, 0.001),
        (LengthUnitSystem::Kilometers, 1e-6),
        (LengthUnitSystem::Microns, 1000.),
    ] {
        let imported =
            read_step_planar_instances_in_units(Cursor::new(&text), &target, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(imported.report, source.report);
        assert_eq!(imported.instances.len(), source.instances.len());
        for (actual, original) in imported.instances.iter().zip(&source.instances) {
            assert_eq!(
                (
                    actual.placement_index,
                    actual.source_shape_id,
                    actual.source_shell_id,
                    &actual.name
                ),
                (
                    original.placement_index,
                    original.source_shape_id,
                    original.source_shell_id,
                    &original.name
                )
            );
            for (vertex, old) in actual.brep.vertices().iter().zip(original.brep.vertices()) {
                for (coordinate, expected) in vertex
                    .point()
                    .to_array()
                    .into_iter()
                    .zip(old.point().to_array().map(|v| v * scale))
                {
                    assert!((coordinate - expected).abs() <= expected.abs() * 1e-12);
                }
            }
            for (face, old) in actual.brep.faces().iter().zip(original.brep.faces()) {
                assert_eq!(face.loops(), old.loops());
                assert_eq!(face.is_reversed(), old.is_reversed());
            }
            assert!(
                (actual.brep.area(Tolerance::DEFAULT).unwrap() / (286. * scale * scale) - 1.).abs()
                    < 1e-9
            );
            assert!(
                (actual.brep.signed_volume(Tolerance::DEFAULT).unwrap()
                    / (315. * scale * scale * scale)
                    - 1.)
                    .abs()
                    < 1e-9
            );
        }
    }
}

#[test]
fn native_instance_units_reject_invalid_units_and_preserve_unitless_coordinates() {
    let text = assembly_step(None);
    assert_eq!(
        read_step_planar_instances_in_units(
            Cursor::new(&text),
            &LengthUnitSystem::None,
            Tolerance::DEFAULT
        )
        .unwrap(),
        read_step_planar_instances(Cursor::new(&text), Tolerance::DEFAULT).unwrap()
    );
    assert!(matches!(
        read_step_planar_instances_in_units(
            Cursor::new(&text),
            &LengthUnitSystem::Unset,
            Tolerance::DEFAULT
        ),
        Err(StepError::Units(_))
    ));
    let missing = text.replace("SI_UNIT(.MILLI.,.METRE.)", "SI_UNIT($,.RADIAN.)");
    assert!(matches!(
        read_step_planar_instances_in_units(
            Cursor::new(missing),
            &LengthUnitSystem::Meters,
            Tolerance::DEFAULT
        ),
        Err(StepError::InvalidLengthUnits(_))
    ));
}

#[test]
fn native_planar_instances_keep_solid_void_shells_and_their_sense() {
    use monstertruck::modeling::{Shell, Solid};
    let cube = |radius: f64| -> Solid {
        primitive::cuboid(BoundingBox::from_iter([
            TruckPoint3::new(-radius, -radius, -radius),
            TruckPoint3::new(radius, radius, radius),
        ]))
    };
    let outer = cube(5.);
    let inner = cube(1.);
    let cavity = Shell::from(
        inner.boundaries()[0]
            .iter()
            .map(|face| face.inverse())
            .collect::<Vec<_>>(),
    );
    let solid = Solid::new(vec![outer.boundaries()[0].clone(), cavity]).compress();
    let text = CompleteStepDisplay::new(
        TruckStepModel::from(&solid),
        StepHeaderDescriptor::default(),
    )
    .to_string();
    let imported = read_step_planar_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.instances.len(), 2);
    assert_eq!(
        imported.instances[0].placement_index,
        imported.instances[1].placement_index
    );
    assert_eq!(
        imported.instances[0].source_shape_id,
        imported.instances[1].source_shape_id
    );
    assert_ne!(
        imported.instances[0].source_shell_id,
        imported.instances[1].source_shell_id
    );
    for (instance, expected) in imported.instances.iter().zip([1000., -8.]) {
        assert!((instance.brep.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-9);
    }
}

#[test]
fn native_planar_instances_honor_oriented_surface_model_shells() {
    let source = polygon_face_step(&[vec![[0., 0.], [10., 0.], [10., 10.], [0., 10.]]], false);
    let table = Table::from_step(&source).unwrap();
    let shape = *table.shell_based_surface_model.keys().next().unwrap();
    let shell = *table.shell.keys().next().unwrap();
    let original = read_step_planar_instances(Cursor::new(&source), Tolerance::DEFAULT).unwrap();
    for (sense, reversed) in [(".T.", false), (".F.", true)] {
        let mut text = source
            .lines()
            .map(|line| {
                if line.contains("SHELL_BASED_SURFACE_MODEL(") {
                    format!("#{shape} = SHELL_BASED_SURFACE_MODEL('', (#999999));")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let end = text.rfind("ENDSEC;").unwrap();
        text.insert_str(
            end,
            &format!("#999999 = ORIENTED_OPEN_SHELL('', *, #{shell}, {sense});\n"),
        );
        let parsed = Table::from_step(&text).unwrap();
        assert_eq!(parsed.entity_report.total(), 0);
        let imported = read_step_planar_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.instances.len(), 1);
        let instance = &imported.instances[0];
        assert_eq!(
            (instance.source_shape_id, instance.source_shell_id),
            (shape, 999999)
        );
        assert_eq!(instance.brep.faces()[0].is_reversed(), reversed);
        assert_eq!(
            instance.brep.faces()[0].loops(),
            original.instances[0].brep.faces()[0].loops()
        );
        assert!((instance.brep.area(Tolerance::DEFAULT).unwrap() - 100.).abs() < 1e-10);
    }
}

#[test]
fn native_planar_instances_preserve_nested_placements_and_shared_source_ids() {
    for nested in [false, true] {
        let parent = nested.then_some(Matrix4::new(
            0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 0., 100., 200., 300., 1.,
        ));
        let imported =
            read_step_planar_instances(Cursor::new(assembly_step(parent)), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(imported.instances.len(), 3);
        assert_eq!(imported.report.unplaced_shape_count, 0);
        assert!(imported.report.assembly_warning.is_none());
        let source_shape = imported.instances[0].source_shape_id;
        let source_shell = imported.instances[0].source_shell_id;
        let mut placements = imported
            .instances
            .iter()
            .map(|instance| instance.placement_index)
            .collect::<Vec<_>>();
        placements.sort_unstable();
        placements.dedup();
        assert_eq!(placements.len(), 3);
        for i in 0..3 {
            let name = format!("instance {i}");
            let instance = imported
                .instances
                .iter()
                .find(|instance| instance.name.as_deref() == Some(&name))
                .unwrap();
            assert_eq!(instance.source_shape_id, source_shape);
            assert_eq!(instance.source_shell_id, source_shell);
            let brep = &instance.brep;
            assert_eq!(
                (
                    brep.vertices().len(),
                    brep.edges().len(),
                    brep.faces().len()
                ),
                (8, 12, 6)
            );
            for x in [-1., 4.] {
                for y in [-2., 5.] {
                    for z in [-3., 6.] {
                        let expected = if nested {
                            [100. - y, 200. + x + 10. * f64::from(i), 300. + z]
                        } else {
                            [x + 10. * f64::from(i), y, z]
                        };
                        assert!(
                            brep.vertices()
                                .iter()
                                .any(|v| v.point().to_array() == expected)
                        );
                    }
                }
            }
            assert!((brep.area(Tolerance::DEFAULT).unwrap() - 286.).abs() < 1e-9);
            assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 315.).abs() < 1e-9);
        }
    }
}

#[test]
fn instance_plan_resolves_nested_placements_without_loading_shell_geometry() {
    for nested in [false, true] {
        let parent = nested.then_some(Matrix4::new(
            0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 0., 100., 200., 300., 1.,
        ));
        let mut table = Table::from_step(&assembly_step(parent)).unwrap();
        let shape_id = *table.manifold_solid_brep.keys().next().unwrap();
        let plan = instance_plan::build(&table).unwrap();
        assert_eq!(plan.instances.len(), 3);
        assert_eq!(plan.report.unplaced_shape_count, 0);
        assert_eq!(plan.report.swallowed_entity_count, 0);
        assert!(plan.report.assembly_warning.is_none());
        for i in 0..3 {
            let name = format!("instance {i}");
            let instance = plan
                .instances
                .iter()
                .find(|instance| instance.name.as_deref() == Some(&name))
                .unwrap();
            assert_eq!(instance.shape_id, shape_id);
            for x in [-1., 4.] {
                for y in [-2., 5.] {
                    for z in [-3., 6.] {
                        let point = instance
                            .transform
                            .transform_point(TruckPoint3::new(x, y, z));
                        let expected = if nested {
                            [100. - y, 200. + x + 10. * f64::from(i), 300. + z]
                        } else {
                            [x + 10. * f64::from(i), y, z]
                        };
                        assert_eq!([point.x, point.y, point.z], expected);
                    }
                }
            }
        }
        // A placement plan depends on shape references, not whether the shell
        // can be decoded or tessellated. Geometry validation belongs to its consumer.
        table.shell.clear();
        assert_eq!(instance_plan::build(&table).unwrap(), plan);
    }
}

#[test]
fn repeated_assembly_instances_preserve_names_and_transforms() {
    let text = assembly_step(None);
    let table = Table::from_step(&text).unwrap();
    assert_eq!(table.manifold_solid_brep.len(), 1);
    let imported = read_step(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects.len(), 3);
    assert_eq!(imported.report.unplaced_shape_count, 0);
    assert!(imported.report.assembly_warning.is_none());
    assert_eq!(imported.report.swallowed_entity_count, 0);
    assert_eq!(imported.report.lost_topology_item_count, 0);
    for index in 0..3 {
        let name = format!("instance {index}");
        let mesh = &imported
            .objects
            .iter()
            .find(|object| object.name.as_deref() == Some(&name))
            .unwrap()
            .mesh;
        assert_eq!(mesh.triangles().len(), 12);
        assert!(mesh.bounds().min().is_near(
            Point3::try_new(-1.0 + 10.0 * index as f64, -2.0, -3.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert!(mesh.bounds().max().is_near(
            Point3::try_new(4.0 + 10.0 * index as f64, 5.0, 6.0).unwrap(),
            Tolerance::DEFAULT
        ));
    }
}

#[test]
fn nested_assembly_composes_noncommuting_transforms_parent_first() {
    // Parent maps (x, y, z) to (100-y, 200+x, 300+z).
    // Child translates by (10*i, 0, 0) in that parent's local frame.
    let parent = Matrix4::new(
        0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 100.0, 200.0, 300.0, 1.0,
    );
    let text = assembly_step(Some(parent));
    let imported = read_step(Cursor::new(text), Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects.len(), 3);
    assert_eq!(imported.report.unplaced_shape_count, 0);
    assert!(imported.report.assembly_warning.is_none());
    for index in 0..3 {
        let name = format!("instance {index}");
        let mesh = &imported
            .objects
            .iter()
            .find(|object| object.name.as_deref() == Some(&name))
            .unwrap()
            .mesh;
        let offset = 10.0 * index as f64;
        assert!(mesh.bounds().min().is_near(
            Point3::try_new(95.0, 199.0 + offset, 297.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert!(mesh.bounds().max().is_near(
            Point3::try_new(102.0, 204.0 + offset, 306.0).unwrap(),
            Tolerance::DEFAULT
        ));
        assert_eq!(mesh.triangles().len(), 12);
        // Every vertex must remain a transformed corner, not merely lie in
        // a correct bounding box. Expected coordinates use no matrix code.
        for point in mesh.vertices() {
            for (coordinate, choices) in [
                (point.x(), [95.0, 102.0]),
                (point.y(), [199.0 + offset, 204.0 + offset]),
                (point.z(), [297.0, 306.0]),
            ] {
                assert!(
                    choices
                        .iter()
                        .any(|expected| (coordinate - expected).abs() < 1.0e-8)
                );
            }
        }
    }
}

#[test]
fn cached_source_is_reused_but_each_instance_is_validated() {
    let mut table = Table::from_step(&cube_step()).unwrap();
    let shape = *table.manifold_solid_brep.keys().next().unwrap();
    let mut report = StepImportReport::default();
    let mut cache = BTreeMap::new();
    let original = import_shape(
        &table,
        shape,
        Matrix4::identity(),
        Tolerance::DEFAULT,
        &mut report,
        &mut cache,
    )
    .unwrap()
    .unwrap();
    // Removing the source makes any accidental second tessellation observable.
    table.manifold_solid_brep.remove(&shape);
    let enlarged = import_shape(
        &table,
        shape,
        Matrix4::from_scale(2.0),
        Tolerance::DEFAULT,
        &mut report,
        &mut cache,
    )
    .unwrap()
    .unwrap();
    assert_eq!(enlarged.triangles(), original.triangles());
    for (actual, source) in enlarged.vertices().iter().zip(original.vertices()) {
        assert_eq!(
            *actual,
            Point3::try_new(2.0 * source.x(), 2.0 * source.y(), 2.0 * source.z()).unwrap()
        );
    }
    assert!(
        import_shape(
            &table,
            shape,
            Matrix4::from_scale(0.0),
            Tolerance::DEFAULT,
            &mut report,
            &mut cache
        )
        .is_err()
    );
    assert!(
        import_shape(
            &table,
            shape,
            Matrix4::identity(),
            Tolerance::DEFAULT,
            &mut report,
            &mut cache
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn exported_mesh_round_trips_as_a_shared_edge_step_shell() {
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_step(&mut bytes, std::slice::from_ref(&mesh)).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.starts_with("ISO-10303-21;"));
    assert!(text.contains("SHELL_BASED_SURFACE_MODEL"));
    assert_eq!(text.matches("EDGE_CURVE(").count(), 5);
    assert_eq!(text.matches("ADVANCED_FACE(").count(), 2);

    let imported = read_step(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
    assert_eq!(imported.objects.len(), 1);
    assert_eq!(imported.objects[0].mesh.triangles().len(), 2);
    assert_eq!(imported.objects[0].mesh.bounds(), mesh.bounds());
}

#[test]
fn failed_step_export_does_not_replace_an_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.step");
    std::fs::write(&path, b"keep me").unwrap();

    assert!(matches!(
        write_step_file(&path, &[]),
        Err(StepError::NoMeshesToWrite)
    ));
    assert_eq!(std::fs::read(path).unwrap(), b"keep me");
}

#[test]
fn rejects_invalid_and_geometry_free_step_data() {
    assert!(matches!(
        read_step(Cursor::new(b"not STEP"), Tolerance::DEFAULT),
        Err(StepError::Load(_))
    ));

    let empty = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','','',(''),(''),'','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
    assert!(matches!(
        read_step(Cursor::new(empty), Tolerance::DEFAULT),
        Err(StepError::NoSupportedGeometry)
    ));
}
