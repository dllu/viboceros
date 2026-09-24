//! Model-space volume selection, independent of the active viewport.

use super::*;
use viboceros_geometry::{BoundingBox3, Circle3};

const SEL_VOLUME_SPHERE_USAGE: &str =
    "SelVolumeSphere center radius [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
const CURVE_SAMPLES: usize = 128;
const SURFACE_SAMPLES_PER_SPAN: usize = 16;

pub(super) struct SelVolumeSphereCommand;

impl Command for SelVolumeSphereCommand {
    fn name(&self) -> &'static str {
        "SelVolumeSphere"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (center, consumed) = parse_point(arguments)?;
        let Some(radius) = arguments.get(consumed) else {
            return Err(CommandError::Usage(SEL_VOLUME_SPHERE_USAGE));
        };
        let radius = radius
            .parse::<Real>()
            .map_err(|_| CommandError::InvalidNumber((*radius).to_owned()))?;
        if !radius.is_finite() || radius <= 0.0 {
            return Err(CommandError::Usage(SEL_VOLUME_SPHERE_USAGE));
        }
        let mode = match &arguments[consumed + 1..] {
            [] => interface::RectSelectionMode::Crossing,
            [option] => {
                let value = option
                    .split_once('=')
                    .filter(|(name, _)| {
                        name.trim_start_matches('_')
                            .eq_ignore_ascii_case("SelectionMode")
                    })
                    .map_or(*option, |(_, value)| value);
                interface::RectSelectionMode::parse(value.trim_start_matches('_'))
                    .ok_or(CommandError::Usage(SEL_VOLUME_SPHERE_USAGE))?
            }
            _ => return Err(CommandError::Usage(SEL_VOLUME_SPHERE_USAGE)),
        };
        let sphere = SelectionSphere { center, radius };
        let mut ids = Vec::new();
        for object in document.selectable_objects() {
            if let Geometry::PointCloud(cloud) = object.geometry()
                && cloud.hidden_count() == cloud.points().len()
            {
                continue;
            }
            let relation = sphere.classify(object.geometry(), document.tolerance())?;
            let selected = match (mode.crossing(true), mode.inverted()) {
                (false, false) => relation.window,
                (true, false) => relation.crossing,
                (false, true) => !relation.crossing,
                (true, true) => !relation.window,
            };
            if selected {
                ids.push(object.id());
            }
        }
        let count = document.select_objects(ids, SelectionMode::Replace)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SphereRelation {
    window: bool,
    crossing: bool,
}

impl SphereRelation {
    const INSIDE: Self = Self {
        window: true,
        crossing: true,
    };
    const OUTSIDE: Self = Self {
        window: false,
        crossing: false,
    };
}

#[derive(Clone, Copy)]
struct SelectionSphere {
    center: Point3,
    radius: Real,
}

impl SelectionSphere {
    fn contains(self, point: Point3) -> bool {
        let center = self.center.to_array();
        let point = point.to_array();
        let delta = [
            center[0] - point[0],
            center[1] - point[1],
            center[2] - point[2],
        ];
        delta[0].hypot(delta[1]).hypot(delta[2]) <= self.radius
    }

    fn classify(
        self,
        geometry: &Geometry,
        tolerance: Tolerance,
    ) -> Result<SphereRelation, CommandError> {
        if !matches!(geometry, Geometry::PointCloud(_))
            && let Some(relation) = self.classify_bounds(geometry.bounds())
        {
            return Ok(relation);
        }
        match geometry {
            Geometry::Point(point) => Ok(if self.contains(*point) {
                SphereRelation::INSIDE
            } else {
                SphereRelation::OUTSIDE
            }),
            Geometry::PointCloud(cloud) => self.classify_points(
                cloud
                    .points()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, point)| (!cloud.is_hidden(i)).then_some(*point)),
            ),
            Geometry::Line(line) => self.classify_segment(line.start(), line.end()),
            Geometry::Polyline(polyline) => self.classify_polyline(polyline.vertices()),
            Geometry::Circle(circle) => self.classify_circle(*circle),
            Geometry::Mesh(mesh) => self.classify_mesh(mesh),
            Geometry::NurbsSurface(surface) => {
                self.classify_mesh(&surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)?)
            }
            Geometry::Brep(brep) => {
                self.classify_mesh(&brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)?)
            }
            Geometry::Arc(_)
            | Geometry::Ellipse(_)
            | Geometry::NurbsCurve(_)
            | Geometry::PolyCurve(_) => {
                let curve = geometry.curve_ref().expect("curve geometry");
                let points = curve.sample_equal_length_points(CURVE_SAMPLES, true, tolerance)?;
                self.classify_polyline(&points)
            }
        }
    }

    /// A bounding box entirely in or disjoint from the sphere settles the
    /// classification without sampling the object's representation.
    fn classify_bounds(self, bounds: BoundingBox3) -> Option<SphereRelation> {
        let center = self.center.to_array();
        let min = bounds.min().to_array();
        let max = bounds.max().to_array();
        let mut nearest = [0.0; 3];
        let mut farthest = [0.0; 3];
        for axis in 0..3 {
            nearest[axis] = if center[axis] < min[axis] {
                min[axis] - center[axis]
            } else if center[axis] > max[axis] {
                center[axis] - max[axis]
            } else {
                0.0
            };
            farthest[axis] = (center[axis] - min[axis])
                .abs()
                .max((center[axis] - max[axis]).abs());
        }
        let norm = |v: [Real; 3]| v[0].hypot(v[1]).hypot(v[2]);
        if norm(nearest) > self.radius {
            Some(SphereRelation::OUTSIDE)
        } else if norm(farthest) <= self.radius {
            Some(SphereRelation::INSIDE)
        } else {
            None
        }
    }

    fn classify_points(
        self,
        points: impl IntoIterator<Item = Point3>,
    ) -> Result<SphereRelation, CommandError> {
        let mut any = false;
        let mut window = true;
        let mut crossing = false;
        for point in points {
            any = true;
            let inside = self.contains(point);
            window &= inside;
            crossing |= inside;
        }
        Ok(SphereRelation {
            window: any && window,
            crossing,
        })
    }

    fn classify_segment(self, start: Point3, end: Point3) -> Result<SphereRelation, CommandError> {
        let start_inside = self.contains(start);
        let end_inside = self.contains(end);
        if start_inside && end_inside {
            return Ok(SphereRelation::INSIDE);
        }
        let a = start.to_array();
        let b = end.to_array();
        let c = self.center.to_array();
        let raw_direction = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let raw_offset = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let local_scale = raw_direction
            .iter()
            .copied()
            .map(Real::abs)
            .fold(0.0, Real::max);
        let (direction, offset) = if local_scale > 0.0
            && local_scale.is_finite()
            && raw_offset
                .iter()
                .all(|value| (value / local_scale).is_finite())
        {
            (
                raw_direction.map(|value| value / local_scale),
                raw_offset.map(|value| value / local_scale),
            )
        } else {
            // Opposite extreme finite endpoints can overflow when subtracted.
            let scale = a
                .into_iter()
                .chain(b)
                .chain(c)
                .map(Real::abs)
                .fold(0.0, Real::max);
            (
                [
                    b[0] / scale - a[0] / scale,
                    b[1] / scale - a[1] / scale,
                    b[2] / scale - a[2] / scale,
                ],
                [
                    c[0] / scale - a[0] / scale,
                    c[1] / scale - a[1] / scale,
                    c[2] / scale - a[2] / scale,
                ],
            )
        };
        let t = {
            let denominator = direction.iter().map(|value| value * value).sum::<Real>();
            if denominator > 0.0 {
                let numerator = direction
                    .iter()
                    .zip(offset)
                    .map(|(direction, offset)| direction * offset)
                    .sum::<Real>();
                (numerator / denominator).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        let closest = Point3::try_new(
            (1.0 - t).mul_add(a[0], t * b[0]),
            (1.0 - t).mul_add(a[1], t * b[1]),
            (1.0 - t).mul_add(a[2], t * b[2]),
        )?;
        Ok(SphereRelation {
            window: false,
            crossing: start_inside || end_inside || self.contains(closest),
        })
    }

    fn classify_polyline(self, points: &[Point3]) -> Result<SphereRelation, CommandError> {
        let vertices = self.classify_points(points.iter().copied())?;
        if vertices.window || points.len() < 2 {
            return Ok(vertices);
        }
        let mut crossing = vertices.crossing;
        for pair in points.windows(2) {
            crossing |= self.classify_segment(pair[0], pair[1])?.crossing;
            if crossing {
                break;
            }
        }
        Ok(SphereRelation {
            window: false,
            crossing,
        })
    }

    fn classify_circle(self, circle: Circle3) -> Result<SphereRelation, CommandError> {
        let center = self.center.to_array();
        let origin = circle.center().to_array();
        let offset = [
            center[0] - origin[0],
            center[1] - origin[1],
            center[2] - origin[2],
        ];
        let dot = |axis: viboceros_geometry::UnitVector3| {
            axis.x()
                .mul_add(offset[0], axis.y().mul_add(offset[1], axis.z() * offset[2]))
        };
        let planar = dot(circle.x_axis()).hypot(dot(circle.y_axis()));
        let height = dot(circle.normal()?).abs();
        Ok(SphereRelation {
            window: height.hypot(planar + circle.radius()) <= self.radius,
            crossing: height.hypot((planar - circle.radius()).abs()) <= self.radius,
        })
    }

    fn classify_mesh(
        self,
        mesh: &viboceros_geometry::TriangleMesh,
    ) -> Result<SphereRelation, CommandError> {
        let vertices = self.classify_points(mesh.vertices().iter().copied())?;
        if vertices.window || vertices.crossing {
            return Ok(vertices);
        }
        for index in 0..mesh.faces().len() {
            if self.contains(mesh.closest_point_on_face(index, self.center)?) {
                return Ok(SphereRelation {
                    window: false,
                    crossing: true,
                });
            }
        }
        Ok(SphereRelation::OUTSIDE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{LineSegment, PointCloud3, TriangleMesh, UnitVector3};

    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn sphere_handles_tangent_circle_long_segment_and_face_without_inside_vertices() {
        let sphere = SelectionSphere {
            center: p(0.0, 0.0, 0.0),
            radius: 1.0,
        };
        let line = Geometry::Line(
            LineSegment::try_new(p(-1e150, 0.0, 0.0), p(1e150, 0.0, 0.0), Tolerance::DEFAULT)
                .unwrap(),
        );
        let relation = sphere.classify(&line, Tolerance::DEFAULT).unwrap();
        assert!(!relation.window && relation.crossing);
        let distant_origin = SelectionSphere {
            center: p(0.0, 1e308, 0.0),
            radius: 1.0,
        };
        let offset_line = Geometry::Line(
            LineSegment::try_new(
                p(-1e100, 1e308, 0.0),
                p(1e100, 1e308, 0.0),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        assert!(
            distant_origin
                .classify(&offset_line, Tolerance::DEFAULT)
                .unwrap()
                .crossing
        );
        let tangent_line = Geometry::Line(
            LineSegment::try_new(p(-2.0, 1.0, 0.0), p(2.0, 1.0, 0.0), Tolerance::DEFAULT).unwrap(),
        );
        assert!(
            sphere
                .classify(&tangent_line, Tolerance::DEFAULT)
                .unwrap()
                .crossing
        );

        let tangent = Geometry::Circle(
            Circle3::try_new(
                p(2.0, 0.0, 0.0),
                1.0,
                UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        assert_eq!(
            sphere.classify(&tangent, Tolerance::DEFAULT).unwrap(),
            SphereRelation {
                window: false,
                crossing: true,
            }
        );

        let mesh = Geometry::Mesh(
            TriangleMesh::try_new(
                vec![p(-4.0, -4.0, 0.0), p(4.0, -4.0, 0.0), p(0.0, 4.0, 0.0)],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
        assert_eq!(
            sphere.classify(&mesh, Tolerance::DEFAULT).unwrap(),
            SphereRelation {
                window: false,
                crossing: true,
            }
        );
    }

    #[test]
    fn hidden_point_cloud_members_do_not_make_a_volume_hit() {
        let cloud = PointCloud3::try_new(vec![p(0.0, 0.0, 0.0), p(5.0, 0.0, 0.0)])
            .unwrap()
            .with_hidden(vec![true, false])
            .unwrap();
        let sphere = SelectionSphere {
            center: p(0.0, 0.0, 0.0),
            radius: 1.0,
        };
        assert_eq!(
            sphere
                .classify(&Geometry::PointCloud(cloud), Tolerance::DEFAULT)
                .unwrap(),
            SphereRelation::OUTSIDE
        );
    }

    #[test]
    fn planar_surface_crosses_sphere_even_with_all_corners_outside() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "SrfPt -2,-2,0 2,-2,0 2,2,0 -2,2,0")
            .unwrap();
        let surface = document.objects().next().unwrap();
        assert!(matches!(surface.geometry(), Geometry::NurbsSurface(_)));
        registry
            .execute(&mut document, "SelVolumeSphere 0,0,0 1")
            .unwrap();
        assert_eq!(document.selected_object_count(), 1);
        registry
            .execute(
                &mut document,
                "SelVolumeSphere 0,0,0 1 SelectionMode=Window",
            )
            .unwrap();
        assert_eq!(document.selected_object_count(), 0);
    }

    #[test]
    fn command_selects_all_four_sphere_modes_without_changing_model_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let inside = document
            .add_geometry(Geometry::Point(p(0.0, 0.0, 0.0)))
            .unwrap();
        let outside = document
            .add_geometry(Geometry::Point(p(5.0, 0.0, 0.0)))
            .unwrap();
        let crossing = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(-2.0, 0.0, 0.0), p(2.0, 0.0, 0.0), Tolerance::DEFAULT)
                    .unwrap(),
            ))
            .unwrap();
        let hidden_cloud = PointCloud3::try_new(vec![p(0.0, 0.0, 0.0)])
            .unwrap()
            .with_hidden(vec![true])
            .unwrap();
        document
            .add_geometry(Geometry::PointCloud(hidden_cloud))
            .unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        let undo = document.undo_label().map(str::to_owned);
        for (mode, expected) in [
            ("Window", vec![inside]),
            ("Crossing", vec![inside, crossing]),
            ("InvertWindow", vec![outside]),
            ("InvertCrossing", vec![outside, crossing]),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("SelVolumeSphere 0,0,0 1 SelectionMode={mode}"),
                )
                .unwrap();
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                expected.into_iter().collect(),
            );
        }
        registry
            .execute(&mut document, "SelVolumeSphere 0,0,0 1")
            .unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([inside, crossing])
        );
        registry
            .execute(
                &mut document,
                "SelVolumeSphere 0,0,0 1 SelectionMode=InvertCrossing",
            )
            .unwrap();
        for input in [
            "SelVolumeSphere 0,0,0",
            "SelVolumeSphere 0,0,0 0",
            "SelVolumeSphere 0,0,0 -1",
            "SelVolumeSphere 0,0,0 NaN",
            "SelVolumeSphere 0,0,0 1 SelectionMode=Nope",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                BTreeSet::from([outside, crossing]),
                "{input}"
            );
        }
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        assert_eq!(document.undo_label(), undo.as_deref());
    }
}
