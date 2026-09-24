//! Select against a constant-radius tube around an existing curve.

use super::selection_volume::{VolumeRelation, parse_volume_mode};
use super::*;
use viboceros_geometry::{CurveRef, TriangleMesh};

const USAGE: &str =
    "SelVolumePipe [curve-id] radius [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
const CURVE_SAMPLES: usize = 128;
const SURFACE_SAMPLES_PER_SPAN: usize = 16;

pub(super) struct SelVolumePipeCommand;

impl Command for SelVolumePipeCommand {
    fn name(&self) -> &'static str {
        "SelVolumePipe"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (source_id, remaining) = if let Some(id) = arguments
            .first()
            .and_then(|value| value.parse::<ObjectId>().ok())
        {
            (id, &arguments[1..])
        } else {
            let selected = document.selected_object_ids().collect::<Vec<_>>();
            let [id] = selected.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            (*id, arguments)
        };
        let [radius, options @ ..] = remaining else {
            return Err(CommandError::Usage(USAGE));
        };
        let radius = radius
            .parse::<Real>()
            .map_err(|_| CommandError::InvalidNumber((*radius).to_owned()))?;
        if !radius.is_finite() || radius <= 0.0 {
            return Err(CommandError::Usage(USAGE));
        }
        let mode = match options {
            [] => interface::RectSelectionMode::Crossing,
            [option] => parse_volume_mode(option, USAGE)?,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        if !document.is_object_selectable(source_id) {
            return Err(CommandError::Usage(USAGE));
        }
        let source_geometry = document
            .object(source_id)
            .map(|object| object.geometry())
            .ok_or(CommandError::Usage(USAGE))?;
        let curve = source_geometry
            .curve_ref()
            .ok_or(CommandError::Usage(USAGE))?;
        let tolerance = document.tolerance();
        let path = match source_geometry {
            Geometry::Line(line) => vec![line.start(), line.end()],
            Geometry::Polyline(polyline) => polyline.vertices().to_vec(),
            _ => curve.sample_equal_length_points(CURVE_SAMPLES, true, tolerance)?,
        };
        let tube = SelectionTube {
            curve,
            path,
            bounds: source_geometry.bounds(),
            radius,
            tolerance,
            convex: matches!(source_geometry, Geometry::Line(_)),
        };
        let mut ids = Vec::new();
        for object in document.selectable_objects() {
            if object.id() == source_id {
                continue;
            }
            if let Geometry::PointCloud(cloud) = object.geometry()
                && cloud.hidden_count() == cloud.points().len()
            {
                continue;
            }
            if tube.classify(object.geometry())?.selected(mode) {
                ids.push(object.id());
            }
        }
        let count = document.select_objects(ids, SelectionMode::Replace)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

struct SelectionTube<'a> {
    curve: CurveRef<'a>,
    path: Vec<Point3>,
    bounds: viboceros_geometry::BoundingBox3,
    radius: Real,
    tolerance: Tolerance,
    convex: bool,
}

impl SelectionTube<'_> {
    fn contains(&self, point: Point3) -> Result<bool, CommandError> {
        let parameter = self.curve.closest_parameter(point, self.tolerance)?;
        let nearest = self.curve.evaluate(parameter)?;
        Ok(point.distance_to(nearest)? <= self.radius)
    }

    fn disjoint_bounds(&self, bounds: viboceros_geometry::BoundingBox3) -> bool {
        let source_min = self.bounds.min().to_array();
        let source_max = self.bounds.max().to_array();
        let target_min = bounds.min().to_array();
        let target_max = bounds.max().to_array();
        (0..3).any(|axis| {
            target_min[axis] > source_max[axis] + self.radius
                || target_max[axis] < source_min[axis] - self.radius
        })
    }

    fn classify(&self, geometry: &Geometry) -> Result<VolumeRelation, CommandError> {
        if !matches!(geometry, Geometry::PointCloud(_)) && self.disjoint_bounds(geometry.bounds()) {
            return Ok(VolumeRelation::OUTSIDE);
        }
        match geometry {
            Geometry::Point(point) => Ok(if self.contains(*point)? {
                VolumeRelation::INSIDE
            } else {
                VolumeRelation::OUTSIDE
            }),
            Geometry::PointCloud(cloud) => self.classify_points(
                cloud
                    .points()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, point)| (!cloud.is_hidden(i)).then_some(*point)),
            ),
            Geometry::Line(line) => self.classify_polyline(&[line.start(), line.end()]),
            Geometry::Polyline(polyline) => self.classify_polyline(polyline.vertices()),
            Geometry::Mesh(mesh) => self.classify_mesh(mesh),
            Geometry::NurbsSurface(surface) => {
                self.classify_mesh(&surface.tessellate(SURFACE_SAMPLES_PER_SPAN, self.tolerance)?)
            }
            Geometry::Brep(brep) => {
                self.classify_mesh(&brep.tessellate(SURFACE_SAMPLES_PER_SPAN, self.tolerance)?)
            }
            Geometry::Circle(_)
            | Geometry::Arc(_)
            | Geometry::Ellipse(_)
            | Geometry::NurbsCurve(_)
            | Geometry::PolyCurve(_) => {
                let curve = geometry.curve_ref().expect("curve geometry");
                self.classify_polyline(&curve.sample_equal_length_points(
                    CURVE_SAMPLES,
                    true,
                    self.tolerance,
                )?)
            }
        }
    }

    fn classify_points(
        &self,
        points: impl IntoIterator<Item = Point3>,
    ) -> Result<VolumeRelation, CommandError> {
        let mut any = false;
        let mut window = true;
        let mut crossing = false;
        for point in points {
            any = true;
            let inside = self.contains(point)?;
            window &= inside;
            crossing |= inside;
        }
        Ok(VolumeRelation {
            window: any && window,
            crossing,
        })
    }

    fn classify_polyline(&self, points: &[Point3]) -> Result<VolumeRelation, CommandError> {
        let vertices = self.classify_points(points.iter().copied())?;
        if points.len() < 2 {
            return Ok(vertices);
        }
        let mut window = vertices.window;
        if window && !self.convex {
            for pair in points.windows(2) {
                for step in 1..16 {
                    if !self.contains(interpolate(pair[0], pair[1], step as Real / 16.0)?)? {
                        window = false;
                        break;
                    }
                }
                if !window {
                    break;
                }
            }
        }
        let mut crossing = vertices.crossing;
        if !crossing {
            for target in points.windows(2) {
                for source in self.path.windows(2) {
                    if segment_distance(target[0], target[1], source[0], source[1])? <= self.radius
                    {
                        crossing = true;
                        break;
                    }
                }
                if crossing {
                    break;
                }
            }
        }
        Ok(VolumeRelation { window, crossing })
    }

    fn classify_mesh(&self, mesh: &TriangleMesh) -> Result<VolumeRelation, CommandError> {
        let vertices = self.classify_points(mesh.vertices().iter().copied())?;
        let mut window = vertices.window;
        if window && !self.convex {
            for face in mesh.faces() {
                let indices = face.indices();
                for edge in 0..indices.len() {
                    let a = mesh.vertices()[indices[edge] as usize];
                    let b = mesh.vertices()[indices[(edge + 1) % indices.len()] as usize];
                    for step in 1..16 {
                        if !self.contains(interpolate(a, b, step as Real / 16.0)?)? {
                            window = false;
                            break;
                        }
                    }
                    if !window {
                        break;
                    }
                }
                if !window {
                    break;
                }
            }
        }
        if window && !self.convex {
            for triangle in 0..mesh.triangles().len() {
                let Some([a, b, c]) = mesh.triangle_points(triangle) else {
                    continue;
                };
                let center = Point3::try_new(
                    (a.x() + b.x() + c.x()) / 3.0,
                    (a.y() + b.y() + c.y()) / 3.0,
                    (a.z() + b.z() + c.z()) / 3.0,
                )?;
                if !self.contains(center)? {
                    window = false;
                    break;
                }
            }
        }
        let mut crossing = vertices.crossing;
        if !crossing {
            for (index, face) in mesh.faces().iter().enumerate() {
                let indices = face.indices();
                let corners = [
                    mesh.vertices()[indices[0] as usize],
                    mesh.vertices()[indices[1] as usize],
                    mesh.vertices()[indices[2] as usize],
                    mesh.vertices()[indices[if indices.len() == 4 { 3 } else { 0 }] as usize],
                ];
                for source in self.path.windows(2) {
                    for &endpoint in source {
                        if endpoint.distance_to(mesh.closest_point_on_face(index, endpoint)?)?
                            <= self.radius
                        {
                            crossing = true;
                            break;
                        }
                    }
                    if crossing {
                        break;
                    }
                    for edge in 0..indices.len() {
                        if segment_distance(
                            source[0],
                            source[1],
                            corners[edge],
                            corners[(edge + 1) % indices.len()],
                        )? <= self.radius
                        {
                            crossing = true;
                            break;
                        }
                    }
                    if crossing {
                        break;
                    }
                    for triangle in [[0, 1, 2], [0, 2, 3]] {
                        if triangle[2] >= indices.len() {
                            continue;
                        }
                        if let Some(point) = segment_plane_crossing(
                            source[0],
                            source[1],
                            corners[triangle[0]],
                            corners[triangle[1]],
                            corners[triangle[2]],
                        )? && point.distance_to(mesh.closest_point_on_face(index, point)?)?
                            <= self.radius
                        {
                            crossing = true;
                            break;
                        }
                    }
                    if crossing {
                        break;
                    }
                }
                if crossing {
                    break;
                }
            }
        }
        Ok(VolumeRelation { window, crossing })
    }
}

pub(super) fn interpolate(a: Point3, b: Point3, t: Real) -> Result<Point3, CommandError> {
    let a = a.to_array();
    let b = b.to_array();
    Ok(Point3::try_new(
        (b[0] - a[0]).mul_add(t, a[0]),
        (b[1] - a[1]).mul_add(t, a[1]),
        (b[2] - a[2]).mul_add(t, a[2]),
    )?)
}

pub(super) fn segment_distance(
    a: Point3,
    b: Point3,
    c: Point3,
    d: Point3,
) -> Result<Real, CommandError> {
    let u = a.vector_to(b)?.to_array();
    let v = c.vector_to(d)?.to_array();
    let w = a.vector_to(c)?.to_array();
    let scale = u
        .into_iter()
        .chain(v)
        .chain(w)
        .map(Real::abs)
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(a.distance_to(c)?);
    }
    let u = u.map(|value| value / scale);
    let v = v.map(|value| value / scale);
    let w = w.map(|value| value / scale);
    let dot = |a: [Real; 3], b: [Real; 3]| a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]));
    let aa = dot(u, u);
    let bb = dot(u, v);
    let cc = dot(v, v);
    let dd = dot(u, w);
    let ee = dot(v, w);
    let mut candidates = [(0.0, 0.0); 5];
    let mut count = 0;
    fn push(candidates: &mut [(Real, Real); 5], count: &mut usize, s: Real, t: Real) {
        candidates[*count] = (s, t);
        *count += 1;
    }
    if cc > 0.0 {
        push(&mut candidates, &mut count, 0.0, (-ee / cc).clamp(0.0, 1.0));
        push(
            &mut candidates,
            &mut count,
            1.0,
            ((bb - ee) / cc).clamp(0.0, 1.0),
        );
    }
    if aa > 0.0 {
        push(&mut candidates, &mut count, (dd / aa).clamp(0.0, 1.0), 0.0);
        push(
            &mut candidates,
            &mut count,
            ((dd + bb) / aa).clamp(0.0, 1.0),
            1.0,
        );
    }
    if count == 0 {
        push(&mut candidates, &mut count, 0.0, 0.0);
    }
    let determinant = aa.mul_add(cc, -bb * bb);
    if determinant > 32.0 * Real::EPSILON * aa * cc {
        let s = (dd * cc - bb * ee) / determinant;
        let t = (bb * dd - aa * ee) / determinant;
        if (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t) {
            push(&mut candidates, &mut count, s, t);
        }
    }
    let mut distance = Real::INFINITY;
    for &(s, t) in &candidates[..count] {
        distance = distance.min(interpolate(a, b, s)?.distance_to(interpolate(c, d, t)?)?);
    }
    Ok(distance)
}

fn segment_plane_crossing(
    start: Point3,
    end: Point3,
    a: Point3,
    b: Point3,
    c: Point3,
) -> Result<Option<Point3>, CommandError> {
    let normal = a.vector_to(b)?.cross(a.vector_to(c)?)?;
    let direction = start.vector_to(end)?;
    let denominator = normal.dot(direction)?;
    if denominator == 0.0 {
        return Ok(None);
    }
    let t = normal.dot(start.vector_to(a)?)? / denominator;
    ((0.0..=1.0).contains(&t))
        .then(|| interpolate(start, end, t))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{Circle3, LineSegment, Polyline3, TriangleMesh, UnitVector3};

    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn line(a: Point3, b: Point3) -> Geometry {
        Geometry::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
    }

    #[test]
    fn segment_distance_handles_interior_skew_parallel_and_degenerate_pairs() {
        for (first, second, expected) in [
            (
                [p(-2., 0., 0.), p(2., 0., 0.)],
                [p(0., -2., 0.), p(0., 2., 0.)],
                0.,
            ),
            (
                [p(-2., 0., 0.), p(2., 0., 0.)],
                [p(0., -2., 3.), p(0., 2., 3.)],
                3.,
            ),
            (
                [p(-2., 0., 0.), p(2., 0., 0.)],
                [p(-1., 2., 0.), p(4., 2., 0.)],
                2.,
            ),
            (
                [p(0., 0., 0.), p(0., 0., 0.)],
                [p(2., 0., 0.), p(2., 2., 0.)],
                2.,
            ),
        ] {
            let distance = segment_distance(first[0], first[1], second[0], second[1]).unwrap();
            assert!((distance - expected).abs() < 1e-12);
            let reversed = segment_distance(second[1], second[0], first[1], first[0]).unwrap();
            assert!((reversed - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn selected_line_pipe_classifies_points_segments_and_four_modes() {
        let mut document = Document::default();
        let source = document
            .add_geometry(line(p(0., 0., 0.), p(10., 0., 0.)))
            .unwrap();
        let inside = document
            .add_geometry(Geometry::Point(p(5., 0.5, 0.)))
            .unwrap();
        let cap = document
            .add_geometry(Geometry::Point(p(-0.5, 0., 0.)))
            .unwrap();
        let outside = document
            .add_geometry(Geometry::Point(p(5., 2., 0.)))
            .unwrap();
        let crossing = document
            .add_geometry(line(p(5., 2., 0.), p(5., -2., 0.)))
            .unwrap();
        let enclosed = document
            .add_geometry(line(p(2., 0.2, 0.), p(8., 0.2, 0.)))
            .unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        for (mode, expected) in [
            ("Window", vec![inside, cap, enclosed]),
            ("Crossing", vec![inside, cap, crossing, enclosed]),
            ("InvertWindow", vec![outside]),
            ("InvertCrossing", vec![outside, crossing]),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("SelVolumePipe {source} 1 SelectionMode={mode}"),
                )
                .unwrap();
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                expected.into_iter().collect(),
                "{mode}"
            );
        }
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "SelVolumePipe 1").unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([inside, cap, crossing, enclosed])
        );
    }

    #[test]
    fn curved_source_and_nonconvex_window_use_actual_curve_distance() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Polyline(
                Polyline3::try_new(
                    vec![p(-2., 0., 0.), p(0., 0., 0.), p(0., 2., 0.)],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let gap = document
            .add_geometry(line(p(-1.5, 0., 0.), p(0., 1.5, 0.)))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut document,
                &format!("SelVolumePipe {source} 0.2 SelectionMode=Window"),
            )
            .unwrap();
        assert!(!document.is_selected(gap));
        registry
            .execute(&mut document, &format!("SelVolumePipe {source} 0.2"))
            .unwrap();
        assert!(document.is_selected(gap));

        let circle = Circle3::try_from_frame(
            p(0., 0., 0.),
            2.,
            UnitVector3::try_new(1., 0., 0., Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let circle_id = document.add_geometry(Geometry::Circle(circle)).unwrap();
        let point_id = document
            .add_geometry(Geometry::Point(p(2., 0., 0.)))
            .unwrap();
        registry
            .execute(&mut document, &format!("SelVolumePipe {circle_id} 0.01"))
            .unwrap();
        assert!(document.is_selected(point_id));
        assert!(!document.is_selected(circle_id));
    }

    #[test]
    fn bent_pipe_window_rejects_mesh_with_edge_outside_tube() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Polyline(
                Polyline3::try_new(
                    vec![p(-2., 0., 0.), p(0., 0., 0.), p(0., 2., 0.)],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let mesh = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![p(-1., 0., 0.), p(0., 1., 0.), p(0., 0., 0.)],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut document,
                &format!("SelVolumePipe {source} 0.4 SelectionMode=Window"),
            )
            .unwrap();
        assert!(!document.is_selected(mesh));
        registry
            .execute(&mut document, &format!("SelVolumePipe {source} 0.4"))
            .unwrap();
        assert!(document.is_selected(mesh));
    }

    #[test]
    fn pipe_axis_piercing_face_selects_mesh_without_near_vertices_or_edges() {
        let mut document = Document::default();
        let source = document
            .add_geometry(line(p(0., 0., -5.), p(0., 0., 5.)))
            .unwrap();
        let mesh = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![p(-5., -5., 0.), p(5., -5., 0.), p(0., 5., 0.)],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("SelVolumePipe {source} 0.1"))
            .unwrap();
        assert!(document.is_selected(mesh));
        registry
            .execute(
                &mut document,
                &format!("SelVolumePipe {source} 0.1 SelectionMode=Window"),
            )
            .unwrap();
        assert!(!document.is_selected(mesh));
    }

    #[test]
    fn invalid_pipe_inputs_leave_selection_and_history_unchanged() {
        let mut document = Document::default();
        let source = document
            .add_geometry(line(p(0., 0., 0.), p(1., 0., 0.)))
            .unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        let undo = document.undo_label().map(str::to_owned);
        let registry = CommandRegistry::with_builtins();
        for input in [
            "SelVolumePipe",
            "SelVolumePipe 0",
            "SelVolumePipe -1",
            "SelVolumePipe NaN",
            "SelVolumePipe 1 SelectionMode=Nope",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(
                document.selected_object_ids().collect::<Vec<_>>(),
                vec![source]
            );
        }
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        assert_eq!(document.undo_label(), undo.as_deref());
    }
}
