//! Directional curve/point projection with staged document admission.
use super::*;
#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
#[cfg(feature = "native-smlib")]
use viboceros_geometry::{SurfaceIntersectionInput, SurfaceSurfaceIntersectionEvent};

pub const PROJECT_USAGE: &str = "Project [Sources=id[,id...]] [Targets=id[,id...]] [Direction=CPlaneZ|View|Custom|x,y,z] [DirectionStart=x,y,z DirectionEnd=x,y,z] [DeleteInput=Yes|No] [OutputLayer=Current|Input|TargetObject]";
pub(super) struct ProjectCommand;
#[derive(Clone, Copy)]
enum Layer {
    Current,
    Input,
    Target,
}

fn ids(value: &str) -> Result<Vec<ObjectId>, CommandError> {
    let result = value
        .split(',')
        .map(|p| {
            p.parse::<ObjectId>()
                .map_err(|_| CommandError::Usage(PROJECT_USAGE))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if result.is_empty() {
        return Err(CommandError::Usage(PROJECT_USAGE));
    }
    let mut seen = BTreeSet::new();
    Ok(result.into_iter().filter(|id| seen.insert(*id)).collect())
}

fn extent(
    point: Point3,
    bounds: viboceros_geometry::BoundingBox3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let a = bounds.min().to_array();
    let b = bounds.max().to_array();
    let mut distance = 0_f64;
    for x in [a[0], b[0]] {
        for y in [a[1], b[1]] {
            for z in [a[2], b[2]] {
                distance = distance.max(point.distance_to(Point3::try_new(x, y, z)?)?);
            }
        }
    }
    let distance = distance * 2. + tolerance.absolute() * 4.;
    if !distance.is_finite() || distance <= 0. {
        return Err(GeometryError::CurveIntersectionDidNotConverge);
    }
    Ok(distance)
}

// Control-point bounds enclose a rational image only with common-sign weights.
// Decline other gauges rather than shorten the finite bidirectional sweep.
fn bounded_weights(weights: impl IntoIterator<Item = Real>) -> Result<(), CommandError> {
    let mut sign = None;
    for weight in weights {
        if weight == 0. || sign.is_some_and(|s| s != weight.is_sign_positive()) {
            return Err(CommandError::Usage(
                "Project requires nonzero common-sign rational weights",
            ));
        }
        sign = Some(weight.is_sign_positive());
    }
    Ok(())
}

fn bounded_target(target: &Geometry) -> Result<(), CommandError> {
    match target {
        Geometry::Mesh(_) => Ok(()),
        Geometry::NurbsSurface(s) => bounded_weights(s.control_points().iter().map(|p| p.weight())),
        Geometry::Brep(b) => {
            for f in b.faces() {
                bounded_weights(f.surface().control_points().iter().map(|p| p.weight()))?;
            }
            Ok(())
        }
        _ => Err(CommandError::Usage(PROJECT_USAGE)),
    }
}

fn line(
    point: Point3,
    target: &Geometry,
    direction: Vector3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let distance = extent(point, target.bounds(), tolerance)?;
    NurbsCurve::try_new(
        1,
        vec![
            point.translated(direction.scaled(-distance)?)?,
            point.translated(direction.scaled(distance)?)?,
        ],
        vec![0., 0., 1., 1.],
    )
}

fn projection_plane(
    curve: &NurbsCurve,
    plane: Plane,
    direction: Vector3,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let n = plane.normal().as_vector().to_array();
    let d = direction.to_array();
    let denominator = plane.normal().as_vector().dot(direction)?;
    // A small angle can still give a finite projection inside the target.
    // Classify only an exactly parallel represented direction as a miss.
    if denominator == 0. {
        return Ok(None);
    }
    let matrix = std::array::from_fn(|r| {
        std::array::from_fn(|c| f64::from(r == c) - d[r] * n[c] / denominator)
    });
    let p = plane.origin().to_array();
    let mut translation = [0.; 3];
    for r in 0..3 {
        translation[r] = p[r] - (0..3).map(|c| matrix[r][c] * p[c]).sum::<f64>();
    }
    let projected = curve.transformed(AffineTransform3::try_new(
        matrix,
        Vector3::try_from(translation)?,
    )?)?;
    if projected
        .control_points()
        .iter()
        .all(|p| p.point() == projected.control_points()[0].point())
    {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "projection curve collapses to a point",
        });
    }
    Ok(Some(projected))
}

fn curve_output(
    curve: &NurbsCurve,
    target: &Geometry,
    direction: Vector3,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    bounded_weights(curve.control_points().iter().map(|p| p.weight()))?;
    let faces = match target {
        Geometry::NurbsSurface(s) => vec![Brep::try_surface_face(s.clone(), tolerance)?],
        Geometry::Brep(b) => (0..b.faces().len())
            .map(|i| b.sub_brep(&[i], tolerance))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(CommandError::Usage(PROJECT_USAGE)),
    };
    let mut output = Vec::new();
    for face in faces {
        // The complete planar source needs no kink splitting: this is an exact
        // affine image, clipped with the existing curve/face overlap query.
        if let Some(plane) = face.faces()[0].surface().plane(tolerance)? {
            output.extend(planar_output(
                curve, &face, plane, direction, tolerance, false,
            )?);
            if output.len() > MAX_INTERSECT_OUTPUTS {
                return Err(CommandError::TooManyIntersectOutputs {
                    maximum: MAX_INTERSECT_OUTPUTS,
                });
            }
            continue;
        }
        #[cfg(feature = "native-smlib")]
        {
            // Split source kinks into regular ruled projection surfaces. The
            // source relation C(u)+v*d is exact; coupled intersection certificates
            // validate the whole output against it and the original target face.
            let mut breaks = vec![*curve.domain().start()];
            breaks.extend(curve.full_multiplicity_kink_parameters(tolerance)?);
            breaks.push(*curve.domain().end());
            for pair in breaks.windows(2) {
                let source = curve_segments::trim(curve, pair[0]..=pair[1])?;
                let mut distance = 0_f64;
                for p in source.control_points() {
                    distance = distance.max(extent(p.point(), face.bounds(), tolerance)?);
                }
                let projector = NurbsSurface::try_extruded_curve(
                    &source,
                    direction.scaled(-distance)?,
                    direction.scaled(distance)?,
                )?;
                for event in viboceros_smlib::surface_face_intersection_events(
                    SurfaceIntersectionInput::Face(&face.faces()[0]),
                    SurfaceIntersectionInput::Surface(&projector),
                    tolerance,
                )
                .map_err(knot_refinement::native_geometry_error)?
                {
                    match event {
                        SurfaceSurfaceIntersectionEvent::Curve(c) => {
                            output.push(Geometry::NurbsCurve(c))
                        }
                        SurfaceSurfaceIntersectionEvent::Point(p) => {
                            output.push(Geometry::Point(p))
                        }
                    }
                }
            }
        }
        #[cfg(not(feature = "native-smlib"))]
        return Err(CommandError::Usage(
            "Project curved targets requires native-smlib",
        ));
        #[cfg(feature = "native-smlib")]
        if output.len() > MAX_INTERSECT_OUTPUTS {
            return Err(CommandError::TooManyIntersectOutputs {
                maximum: MAX_INTERSECT_OUTPUTS,
            });
        }
    }
    Ok(output)
}

impl Command for ProjectCommand {
    fn name(&self) -> &'static str {
        "Project"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let mut sources = None;
        let mut targets = None;
        let mut direction = context.construction_plane.z_axis().as_vector();
        let mut view = false;
        let mut custom = false;
        let mut direction_start = None;
        let mut direction_end = None;
        let mut view_direction = None;
        let mut delete = false;
        let mut layer = Layer::Current;
        let mut seen = BTreeSet::new();
        for argument in arguments {
            let (key, value) = argument
                .trim_start_matches('_')
                .split_once('=')
                .ok_or(CommandError::Usage(PROJECT_USAGE))?;
            if !seen.insert(key.to_ascii_lowercase()) {
                return Err(CommandError::Usage(PROJECT_USAGE));
            }
            match key.to_ascii_lowercase().as_str() {
                "sources" => sources = Some(ids(value)?),
                "targets" => targets = Some(ids(value)?),
                "direction" => {
                    direction = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "cplanez" => direction,
                        "view" => {
                            view = true;
                            direction
                        }
                        "custom" => {
                            custom = true;
                            direction
                        }
                        _ => Vector3::try_from(parse_point(&[value])?.0.to_array())?,
                    }
                }
                "viewdirection" => view_direction = Some(parse_axis_option(value, PROJECT_USAGE)?),
                "directionstart" => direction_start = Some(parse_point(&[value])?.0),
                "directionend" => direction_end = Some(parse_point(&[value])?.0),
                "deleteinput" => {
                    delete = parse_yes_no(value).ok_or(CommandError::Usage(PROJECT_USAGE))?
                }
                "outputlayer" => {
                    layer = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "current" => Layer::Current,
                        "input" => Layer::Input,
                        "targetobject" => Layer::Target,
                        _ => return Err(CommandError::Usage(PROJECT_USAGE)),
                    }
                }
                _ => return Err(CommandError::Usage(PROJECT_USAGE)),
            }
        }
        if custom {
            let (start, end) = direction_start.zip(direction_end).ok_or(CommandError::Usage(
                "Project Direction=Custom requires DirectionStart=x,y,z and DirectionEnd=x,y,z outside a point prompt",
            ))?;
            direction = start.direction_to(end)?.as_vector();
        } else if direction_start.is_some() || direction_end.is_some() {
            return Err(CommandError::Usage(PROJECT_USAGE));
        }
        if view {
            direction = view_direction.ok_or(CommandError::Usage(
                "Project Direction=View requires ViewDirection=x,y,z outside a viewport",
            ))?;
        } else if view_direction.is_some() {
            return Err(CommandError::Usage(PROJECT_USAGE));
        }
        direction = direction.normalized_nonzero()?.as_vector();
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let source_ids = sources.unwrap_or_else(|| {
            selected
                .iter()
                .copied()
                .filter(|id| {
                    document.object(*id).is_some_and(|o| {
                        matches!(o.geometry(), Geometry::Point(_))
                            || o.geometry().curve_ref().is_some()
                    })
                })
                .collect()
        });
        let target_ids = targets.unwrap_or_else(|| {
            selected
                .iter()
                .copied()
                .filter(|id| {
                    document.object(*id).is_some_and(|o| {
                        matches!(
                            o.geometry(),
                            Geometry::NurbsSurface(_) | Geometry::Brep(_) | Geometry::Mesh(_)
                        )
                    })
                })
                .collect()
        });
        if source_ids.is_empty()
            || target_ids.is_empty()
            || source_ids.len().saturating_mul(target_ids.len()) > MAX_INTERSECT_PAIRS
        {
            return Err(CommandError::Usage(PROJECT_USAGE));
        }
        // Borrow and build each mesh target once; output admission happens
        // only after these caches and all source queries have been released.
        let mut mesh_indexes = BTreeMap::new();
        for id in &target_ids {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            if !document.is_object_selectable(*id) {
                return Err(CommandError::Usage(PROJECT_USAGE));
            }
            bounded_target(object.geometry())?;
            if let Geometry::Mesh(mesh) = object.geometry() {
                mesh_indexes.insert(*id, viboceros_geometry::MeshProjectionIndex::new(mesh)?);
            }
        }
        let mut staged = Vec::new();
        let tolerance = document.tolerance();
        for source_id in &source_ids {
            let source = document
                .object(*source_id)
                .ok_or(DocumentError::ObjectNotFound(*source_id))?;
            if !document.is_object_selectable(*source_id) {
                return Err(CommandError::Usage(PROJECT_USAGE));
            }
            for target_id in &target_ids {
                if source_id == target_id || !document.is_object_selectable(*target_id) {
                    return Err(CommandError::Usage(PROJECT_USAGE));
                }
                let target = document
                    .object(*target_id)
                    .ok_or(DocumentError::ObjectNotFound(*target_id))?;
                let output = if let Geometry::Point(point) = source.geometry() {
                    if let Some(index) = mesh_indexes.get(target_id) {
                        index
                            .project_point(*point, direction, tolerance)?
                            .into_iter()
                            .map(Geometry::Point)
                            .collect()
                    } else {
                        let ray = line(*point, target.geometry(), direction, tolerance)?;
                        match target.geometry() {
                            Geometry::NurbsSurface(surface) => {
                                curve_intersections::surface_events(&ray, surface, tolerance)?
                                    .into_iter()
                                    .filter_map(|e| match e {
                                        CurveSurfaceIntersectionEvent::Point(p) => {
                                            Some(Geometry::Point(p.point()))
                                        }
                                        _ => None,
                                    })
                                    .collect()
                            }
                            Geometry::Brep(brep) => {
                                curve_intersections::brep_events(&ray, brep, None, tolerance)?
                                    .into_iter()
                                    .filter_map(|e| match e {
                                        CurveBrepIntersectionEvent::Point(p) => {
                                            Some(Geometry::Point(p.point()))
                                        }
                                        _ => None,
                                    })
                                    .collect()
                            }
                            _ => return Err(CommandError::Usage(PROJECT_USAGE)),
                        }
                    }
                } else if let Geometry::Mesh(_) = target.geometry() {
                    mesh_curve_output(
                        &source
                            .geometry()
                            .nurbs_curve_representation()?
                            .ok_or(CommandError::Usage(PROJECT_USAGE))?,
                        &mesh_indexes[target_id],
                        direction,
                        tolerance,
                    )?
                } else {
                    curve_output(
                        &source
                            .geometry()
                            .nurbs_curve_representation()?
                            .ok_or(CommandError::Usage(PROJECT_USAGE))?,
                        target.geometry(),
                        direction,
                        tolerance,
                    )?
                };
                let layer_id = match layer {
                    Layer::Current => document.current_layer_id(),
                    Layer::Input => source.attributes().layer_id(),
                    Layer::Target => target.attributes().layer_id(),
                };
                staged.extend(output.into_iter().map(|g| (g, layer_id)));
                if staged.len() > MAX_INTERSECT_OUTPUTS {
                    return Err(CommandError::TooManyIntersectOutputs {
                        maximum: MAX_INTERSECT_OUTPUTS,
                    });
                }
            }
        }
        drop(mesh_indexes);
        if staged.is_empty() {
            return Ok("Projection missed the target objects".into());
        }
        let mut output_ids = Vec::new();
        for (geometry, layer) in staged {
            output_ids.push(
                document
                    .add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(layer))?,
            );
        }
        if delete {
            document.delete_objects(source_ids.iter().copied())?;
        }
        document.select_command_results(output_ids.iter().copied())?;
        Ok(format!("Created {} projected object(s)", output_ids.len()))
    }
}

fn planar_output(
    curve: &NurbsCurve,
    face: &Brep,
    plane: Plane,
    direction: Vector3,
    tolerance: Tolerance,
    known_plane_image: bool,
) -> Result<Vec<Geometry>, CommandError> {
    let Some(projected) = projection_plane(curve, plane, direction)? else {
        return Ok(vec![]);
    };
    let events = if known_plane_image {
        // The affine image is already planar. Use the established analytic
        // natural-domain query and retain the selected native edge producer;
        // no numerical curve/surface solve is needed to discover that relation.
        viboceros_geometry::curve_brep_intersection_events_with_operations(
            &projected,
            face,
            None,
            tolerance,
            &mut |c, s, t| {
                Ok::<_, CommandError>(viboceros_geometry::curve_surface_intersection_events(
                    c, s, t,
                )?)
            },
            &mut curve_intersections::events,
        )?
    } else {
        curve_intersections::brep_events(&projected, face, None, tolerance)?
    };
    events
        .into_iter()
        .map(|event| match event {
            CurveBrepIntersectionEvent::Overlap(overlap) => Ok(Geometry::NurbsCurve(
                curve_segments::trim(&projected, overlap.curve_interval())?,
            )),
            CurveBrepIntersectionEvent::Point(hit) => Ok(Geometry::Point(hit.point())),
        })
        .collect()
}

fn mesh_curve_output(
    curve: &NurbsCurve,
    index: &viboceros_geometry::MeshProjectionIndex<'_>,
    direction: Vector3,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    bounded_weights(curve.control_points().iter().map(|p| p.weight()))?;
    let mut output = Vec::new();
    for target in index.curve_faces(curve, direction, tolerance)? {
        let plane = target.faces()[0].surface().plane(tolerance)?.ok_or(
            GeometryError::AmbiguousMeshProjection {
                context: "triangle plane preparation",
            },
        )?;
        output.extend(planar_output(
            curve, &target, plane, direction, tolerance, true,
        )?);
        if output.len() > MAX_INTERSECT_OUTPUTS {
            return Err(CommandError::TooManyIntersectOutputs {
                maximum: MAX_INTERSECT_OUTPUTS,
            });
        }
    }
    // Join only exactly identical endpoints. Existing Join's zero-tolerance
    // policy preserves every source endpoint and control image. Approximate
    // contacts remain separate rather than moving certified payloads.
    let mut curves = Vec::new();
    let mut points = Vec::new();
    let mut seen_curves = BTreeSet::new();
    let bits = |n: Real| if n == 0. { 0 } else { n.to_bits() };
    for geometry in output {
        // A curve input produces curve images, as Curve.ProjectToMesh does.
        // Isolated zero-dimensional triangle contacts produce no curve object.
        if let Geometry::NurbsCurve(c) = geometry {
            let mut key = vec![c.degree() as u64, c.control_points().len() as u64];
            key.extend(
                c.control_points()
                    .iter()
                    .flat_map(|p| p.point().to_array().into_iter().chain([p.weight()]))
                    .map(bits),
            );
            key.extend(c.knots().iter().copied().map(bits));
            if seen_curves.insert(key) {
                curves.push(viboceros_geometry::Curve3::NurbsCurve(c));
            }
        }
    }
    for joined in viboceros_geometry::join_curves_with_endpoint_filter(
        &curves,
        viboceros_geometry::CurveJoinOptions {
            tolerance: 0.,
            preserve_direction: true,
            style: viboceros_geometry::CurveJoinStyle::Batch,
        },
        tolerance,
        |a, start_a, b, start_b| {
            let interval_a = curves[a].as_ref().domain();
            let interval_b = curves[b].as_ref().domain();
            let a = if start_a {
                *interval_a.start()
            } else {
                *interval_a.end()
            };
            let b = if start_b {
                *interval_b.start()
            } else {
                *interval_b.end()
            };
            a == b
                || (a == *curve.domain().start() && b == *curve.domain().end())
                || (b == *curve.domain().start() && a == *curve.domain().end())
        },
    )? {
        points.push(Geometry::from(joined.into_curve()));
    }
    Ok(points)
}
