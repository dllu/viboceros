use viboceros_geometry::{GeometryError, MeshFace, NurbsCurve, Point3, Tolerance};

use super::Geometry;

// OpenNURBS' scale-aware zero policy used by Rhino geometry-value
// comparisons. This is intentionally independent of document tolerance.
const GEOMETRY_EQUALITY_SQRT_EPSILON: f64 = 1.490_116_119_385e-8;

impl Geometry {
    /// Compares geometry by Rhino-compatible shape value while ignoring
    /// object attributes.
    ///
    /// Curve direction is ignored, but the natural seam of a closed
    /// piecewise-linear or NURBS curve remains significant, matching Rhino's
    /// duplicate-object selection. Points and meshes compare stored values;
    /// point-cloud locations and supported curves use OpenNURBS' scale-aware
    /// fixed zero policy rather than document tolerance. Degree-one NURBS
    /// control polygons compare with native lines and polylines because
    /// positive weights and knot spacing change only parameterization, not the
    /// traced shape.
    pub fn geometrically_equals(&self, other: &Self) -> Result<bool, GeometryError> {
        if let (Some(left), Some(right)) = (
            self.piecewise_linear_vertices(),
            other.piecewise_linear_vertices(),
        ) {
            return piecewise_linear_paths_equal(&left, &right);
        }
        if let (Self::PointCloud(left), Self::PointCloud(right)) = (self, other) {
            return Ok(left.points().len() == right.points().len()
                && left
                    .points()
                    .iter()
                    .zip(right.points())
                    .all(|(left, right)| points_equal_with_fixed_zero_policy(*left, *right)));
        }
        if let (Self::Brep(left), Self::Brep(right)) = (self, other) {
            return Ok(left == right);
        }
        if let (Some(left), Some(right)) = (circle_components(self)?, circle_components(other)?) {
            let distance_tolerance =
                GEOMETRY_EQUALITY_SQRT_EPSILON * left.1 + GEOMETRY_EQUALITY_SQRT_EPSILON * right.1;
            return Ok(left.0.distance_to(right.0)? <= distance_tolerance
                && (left.1 - right.1).abs() <= distance_tolerance
                && unoriented_vector_key(left.2) == unoriented_vector_key(right.2));
        }
        if let (Self::Arc(left), Self::Arc(right)) = (self, other) {
            if left == right {
                return Ok(true);
            }
            let reversal_tolerance =
                Tolerance::try_new(f64::MIN_POSITIVE, f64::MIN_POSITIVE, f64::MIN_POSITIVE)?;
            return Ok(left
                .reversed(reversal_tolerance)
                .is_ok_and(|reversed| reversed == *right)
                || right
                    .reversed(reversal_tolerance)
                    .is_ok_and(|reversed| reversed == *left));
        }
        Ok(self.duplicate_key()? == other.duplicate_key()?)
    }

    pub(super) fn duplicate_family(&self) -> DuplicateGeometryFamily {
        match self {
            Self::Point(_) => DuplicateGeometryFamily::Point,
            Self::PointCloud(_) => DuplicateGeometryFamily::PointCloud,
            Self::Line(_) | Self::Polyline(_) => DuplicateGeometryFamily::PiecewiseLinear,
            Self::Circle(_) => DuplicateGeometryFamily::Circle,
            Self::Arc(_) => DuplicateGeometryFamily::Arc,
            Self::Ellipse(ellipse) if ellipse.radius_x() == ellipse.radius_y() => {
                DuplicateGeometryFamily::Circle
            }
            Self::Ellipse(_) => DuplicateGeometryFamily::Ellipse,
            Self::NurbsCurve(curve) if nurbs_piecewise_linear_vertices(curve).is_some() => {
                DuplicateGeometryFamily::PiecewiseLinear
            }
            Self::NurbsCurve(_) => DuplicateGeometryFamily::NurbsCurve,
            Self::NurbsSurface(_) => DuplicateGeometryFamily::NurbsSurface,
            Self::Brep(_) => DuplicateGeometryFamily::Brep,
            Self::Mesh(_) => DuplicateGeometryFamily::Mesh,
        }
    }

    fn piecewise_linear_vertices(&self) -> Option<Vec<Point3>> {
        match self {
            Self::Line(line) => Some(vec![line.start(), line.end()]),
            Self::Polyline(polyline) => Some(polyline.vertices().to_vec()),
            Self::NurbsCurve(curve) => nurbs_piecewise_linear_vertices(curve),
            _ => None,
        }
    }

    fn duplicate_key(&self) -> Result<DuplicateGeometryKey, GeometryError> {
        Ok(match self {
            Self::Point(point) => DuplicateGeometryKey::Point(point_key(*point)),
            Self::PointCloud(cloud) => DuplicateGeometryKey::PointCloud(
                cloud.points().iter().copied().map(point_key).collect(),
            ),
            Self::Line(line) => DuplicateGeometryKey::PiecewiseLinear(canonical_point_path(&[
                line.start(),
                line.end(),
            ])),
            Self::Circle(circle) => circle_key(circle.center(), circle.radius(), circle.normal()?),
            Self::Arc(arc) => DuplicateGeometryKey::ArcCandidates {
                center: point_key(arc.center()),
                radius: real_key(arc.radius()),
                sweep: real_key(arc.sweep_radians()),
            },
            Self::Ellipse(ellipse) if ellipse.radius_x() == ellipse.radius_y() => {
                circle_key(ellipse.center(), ellipse.radius_x(), ellipse.normal()?)
            }
            Self::Ellipse(ellipse) => {
                let mut axes = [
                    (
                        real_key(ellipse.radius_x()),
                        unoriented_vector_key(ellipse.x_axis()),
                    ),
                    (
                        real_key(ellipse.radius_y()),
                        unoriented_vector_key(ellipse.y_axis()),
                    ),
                ];
                axes.sort_unstable();
                DuplicateGeometryKey::Ellipse {
                    center: point_key(ellipse.center()),
                    axes,
                }
            }
            Self::Polyline(polyline) => {
                DuplicateGeometryKey::PiecewiseLinear(canonical_point_path(polyline.vertices()))
            }
            Self::NurbsCurve(curve) => {
                if let Some(vertices) = nurbs_piecewise_linear_vertices(curve) {
                    DuplicateGeometryKey::PiecewiseLinear(canonical_point_path(&vertices))
                } else {
                    DuplicateGeometryKey::NurbsCurve(canonical_nurbs_curve_key(curve))
                }
            }
            Self::NurbsSurface(surface) => DuplicateGeometryKey::NurbsSurface {
                degree_u: surface.degree_u(),
                degree_v: surface.degree_v(),
                count_u: surface.control_point_count_u(),
                count_v: surface.control_point_count_v(),
                controls: normalized_control_keys(surface.control_points()),
                knots_u: normalized_parameter_keys(surface.knots_u()),
                knots_v: normalized_parameter_keys(surface.knots_v()),
            },
            Self::Brep(_) => DuplicateGeometryKey::Brep,
            Self::Mesh(mesh) => DuplicateGeometryKey::Mesh {
                vertices: mesh.vertices().iter().copied().map(point_key).collect(),
                faces: mesh.faces().to_vec(),
            },
        })
    }
}

fn points_equal_with_fixed_zero_policy(left: Point3, right: Point3) -> bool {
    [
        (left.x(), right.x()),
        (left.y(), right.y()),
        (left.z(), right.z()),
    ]
    .into_iter()
    .all(|(left, right)| {
        let difference = (left - right).abs();
        difference <= GEOMETRY_EQUALITY_SQRT_EPSILON
            || difference <= (left.abs() + right.abs()) * GEOMETRY_EQUALITY_SQRT_EPSILON
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum DuplicateGeometryFamily {
    Point,
    PointCloud,
    PiecewiseLinear,
    Circle,
    Arc,
    Ellipse,
    NurbsCurve,
    NurbsSurface,
    Brep,
    Mesh,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DuplicateGeometryKey {
    Point([u64; 3]),
    PointCloud(Vec<[u64; 3]>),
    PiecewiseLinear(Vec<[u64; 3]>),
    Circle {
        center: [u64; 3],
        radius: u64,
        normal: [u64; 3],
    },
    ArcCandidates {
        center: [u64; 3],
        radius: u64,
        sweep: u64,
    },
    Ellipse {
        center: [u64; 3],
        axes: [(u64, [u64; 3]); 2],
    },
    NurbsCurve(NurbsCurveDuplicateKey),
    NurbsSurface {
        degree_u: usize,
        degree_v: usize,
        count_u: usize,
        count_v: usize,
        controls: Vec<([u64; 3], u64)>,
        knots_u: Vec<u64>,
        knots_v: Vec<u64>,
    },
    Brep,
    Mesh {
        vertices: Vec<[u64; 3]>,
        faces: Vec<MeshFace>,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NurbsCurveDuplicateKey {
    degree: usize,
    controls: Vec<([u64; 3], u64)>,
    knots: Vec<u64>,
}

fn circle_key(
    center: Point3,
    radius: f64,
    normal: viboceros_geometry::UnitVector3,
) -> DuplicateGeometryKey {
    DuplicateGeometryKey::Circle {
        center: point_key(center),
        radius: real_key(radius),
        normal: unoriented_vector_key(normal),
    }
}

fn circle_components(
    geometry: &Geometry,
) -> Result<Option<(Point3, f64, viboceros_geometry::UnitVector3)>, GeometryError> {
    match geometry {
        Geometry::Circle(circle) => Ok(Some((circle.center(), circle.radius(), circle.normal()?))),
        Geometry::Ellipse(ellipse) if ellipse.radius_x() == ellipse.radius_y() => Ok(Some((
            ellipse.center(),
            ellipse.radius_x(),
            ellipse.normal()?,
        ))),
        _ => Ok(None),
    }
}

fn piecewise_linear_paths_equal(left: &[Point3], right: &[Point3]) -> Result<bool, GeometryError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    let tolerance = GEOMETRY_EQUALITY_SQRT_EPSILON * saturated_path_length(left)?
        + GEOMETRY_EQUALITY_SQRT_EPSILON * saturated_path_length(right)?;
    Ok(
        paths_match_with_tolerance(left.iter().copied(), right.iter().copied(), tolerance)?
            || paths_match_with_tolerance(
                left.iter().copied(),
                right.iter().rev().copied(),
                tolerance,
            )?,
    )
}

fn saturated_path_length(points: &[Point3]) -> Result<f64, GeometryError> {
    let mut length = 0.0;
    for points in points.windows(2) {
        let segment_length = points[0].distance_to(points[1])?;
        if length > f64::MAX - segment_length {
            return Ok(f64::MAX);
        }
        length += segment_length;
    }
    Ok(length)
}

fn paths_match_with_tolerance(
    left: impl Iterator<Item = Point3>,
    right: impl Iterator<Item = Point3>,
    tolerance: f64,
) -> Result<bool, GeometryError> {
    for (left, right) in left.zip(right) {
        if left.distance_to(right)? > tolerance {
            return Ok(false);
        }
    }
    Ok(true)
}

fn canonical_point_path(points: &[Point3]) -> Vec<[u64; 3]> {
    let forward = points.iter().copied().map(point_key).collect::<Vec<_>>();
    let reversed = forward.iter().rev().copied().collect::<Vec<_>>();
    forward.min(reversed)
}

fn nurbs_piecewise_linear_vertices(curve: &NurbsCurve) -> Option<Vec<Point3>> {
    if curve.degree() != 1 {
        return None;
    }
    let controls = curve.control_points();
    let knots = curve.knots();
    let control_count = controls.len();
    let clamped = knots[0] == knots[1] && knots[control_count] == knots[control_count + 1];
    let every_segment_active = (1..control_count).all(|index| knots[index] < knots[index + 1]);
    let vertices = controls
        .iter()
        .map(|control| control.point())
        .collect::<Vec<_>>();
    let no_degenerate_segments = vertices.windows(2).all(|points| points[0] != points[1]);
    (clamped && every_segment_active && no_degenerate_segments).then_some(vertices)
}

fn canonical_nurbs_curve_key(curve: &NurbsCurve) -> NurbsCurveDuplicateKey {
    let forward = NurbsCurveDuplicateKey {
        degree: curve.degree(),
        controls: normalized_control_keys(curve.control_points()),
        knots: normalized_parameter_keys(curve.knots()),
    };
    let mut reversed_controls = curve.control_points().to_vec();
    reversed_controls.reverse();
    let mut reversed_knots = normalized_parameter_values(curve.knots());
    reversed_knots.reverse();
    for knot in &mut reversed_knots {
        *knot = if *knot == 0.0 {
            1.0
        } else if *knot == 1.0 {
            0.0
        } else {
            1.0 - *knot
        };
    }
    let reversed = NurbsCurveDuplicateKey {
        degree: curve.degree(),
        controls: normalized_control_keys(&reversed_controls),
        knots: reversed_knots.into_iter().map(real_key).collect(),
    };
    forward.min(reversed)
}

fn normalized_control_keys(
    controls: &[viboceros_geometry::WeightedPoint3],
) -> Vec<([u64; 3], u64)> {
    let weight_scale = controls
        .iter()
        .map(|control| control.weight())
        .fold(0.0, f64::max);
    controls
        .iter()
        .map(|control| {
            (
                point_key(control.point()),
                real_key(control.weight() / weight_scale),
            )
        })
        .collect()
}

fn normalized_parameter_keys(parameters: &[f64]) -> Vec<u64> {
    normalized_parameter_values(parameters)
        .into_iter()
        .map(real_key)
        .collect()
}

fn normalized_parameter_values(parameters: &[f64]) -> Vec<f64> {
    let start = parameters[0];
    let end = parameters[parameters.len() - 1];
    let difference = end - start;
    parameters
        .iter()
        .map(|value| {
            if *value == start {
                0.0
            } else if *value == end {
                1.0
            } else if difference.is_finite() {
                (*value - start) / difference
            } else {
                (*value * 0.5 - start * 0.5) / (end * 0.5 - start * 0.5)
            }
        })
        .collect()
}

fn point_key(point: Point3) -> [u64; 3] {
    point.to_array().map(real_key)
}

fn vector_key(vector: viboceros_geometry::UnitVector3) -> [u64; 3] {
    vector.as_vector().to_array().map(real_key)
}

fn unoriented_vector_key(vector: viboceros_geometry::UnitVector3) -> [u64; 3] {
    vector_key(vector).min(vector_key(vector.opposite()))
}

fn real_key(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}
