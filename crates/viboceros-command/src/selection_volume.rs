//! Model-space volume selection, independent of the active viewport.

use super::*;
use viboceros_geometry::{BoundingBox3, Circle3};

const SEL_VOLUME_SPHERE_USAGE: &str =
    "SelVolumeSphere center radius [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
const SEL_BOX_USAGE: &str = "SelBox base-corner opposite-base-corner height [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
const CURVE_SAMPLES: usize = 128;
const SURFACE_SAMPLES_PER_SPAN: usize = 16;

pub(super) struct SelVolumeSphereCommand;
pub(super) struct SelBoxCommand;

impl Command for SelBoxCommand {
    fn name(&self) -> &'static str {
        "SelBox"
    }

    fn records_history(&self) -> bool {
        false
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
        let (base, base_count) = parse_point(arguments)?;
        let (opposite, opposite_count) = parse_point(&arguments[base_count..])?;
        let remaining = &arguments[base_count + opposite_count..];
        let (height, height_count) = if let Some(first) = remaining.first()
            && !first.contains(',')
            && let Ok(height) = first.parse::<Real>()
        {
            (height, 1)
        } else {
            let (height_point, count) = parse_point(remaining)?;
            let frame = context.construction_plane.with_origin(base);
            (frame.coordinates_of(height_point)?[2], count)
        };
        if !height.is_finite() {
            return Err(CommandError::Usage(SEL_BOX_USAGE));
        }
        let mode = match &remaining[height_count..] {
            [] => interface::RectSelectionMode::Crossing,
            [option] => parse_volume_mode(option, SEL_BOX_USAGE)?,
            _ => return Err(CommandError::Usage(SEL_BOX_USAGE)),
        };
        let frame = context.construction_plane.with_origin(base);
        let delta = frame.coordinates_of(opposite)?;
        let intervals = [delta[0], delta[1], height].map(|value| [value.min(0.0), value.max(0.0)]);
        if intervals
            .iter()
            .any(|[low, high]| high - low <= document.tolerance().absolute())
        {
            return Err(CommandError::Usage(SEL_BOX_USAGE));
        }
        let box_region = SelectionBox { frame, intervals };
        let mut ids = Vec::new();
        for object in document.selectable_objects() {
            if let Geometry::PointCloud(cloud) = object.geometry()
                && cloud.hidden_count() == cloud.points().len()
            {
                continue;
            }
            let relation = box_region.classify(object.geometry(), document.tolerance())?;
            if relation.selected(mode) {
                ids.push(object.id());
            }
        }
        let count = document.select_objects(ids, SelectionMode::Replace)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

fn parse_volume_mode(
    option: &str,
    usage: &'static str,
) -> Result<interface::RectSelectionMode, CommandError> {
    let value = option
        .split_once('=')
        .filter(|(name, _)| {
            name.trim_start_matches('_')
                .eq_ignore_ascii_case("SelectionMode")
        })
        .map_or(option, |(_, value)| value);
    interface::RectSelectionMode::parse(value.trim_start_matches('_'))
        .ok_or(CommandError::Usage(usage))
}

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
            [option] => parse_volume_mode(option, SEL_VOLUME_SPHERE_USAGE)?,
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
            if relation.selected(mode) {
                ids.push(object.id());
            }
        }
        let count = document.select_objects(ids, SelectionMode::Replace)?;
        Ok(format!("Selected {count} object(s)"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VolumeRelation {
    window: bool,
    crossing: bool,
}

impl VolumeRelation {
    const INSIDE: Self = Self {
        window: true,
        crossing: true,
    };
    const OUTSIDE: Self = Self {
        window: false,
        crossing: false,
    };

    fn selected(self, mode: interface::RectSelectionMode) -> bool {
        match (mode.crossing(true), mode.inverted()) {
            (false, false) => self.window,
            (true, false) => self.crossing,
            (false, true) => !self.crossing,
            (true, true) => !self.window,
        }
    }
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
    ) -> Result<VolumeRelation, CommandError> {
        if !matches!(geometry, Geometry::PointCloud(_))
            && let Some(relation) = self.classify_bounds(geometry.bounds())
        {
            return Ok(relation);
        }
        match geometry {
            Geometry::Point(point) => Ok(if self.contains(*point) {
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
    fn classify_bounds(self, bounds: BoundingBox3) -> Option<VolumeRelation> {
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
            Some(VolumeRelation::OUTSIDE)
        } else if norm(farthest) <= self.radius {
            Some(VolumeRelation::INSIDE)
        } else {
            None
        }
    }

    fn classify_points(
        self,
        points: impl IntoIterator<Item = Point3>,
    ) -> Result<VolumeRelation, CommandError> {
        let mut any = false;
        let mut window = true;
        let mut crossing = false;
        for point in points {
            any = true;
            let inside = self.contains(point);
            window &= inside;
            crossing |= inside;
        }
        Ok(VolumeRelation {
            window: any && window,
            crossing,
        })
    }

    fn classify_segment(self, start: Point3, end: Point3) -> Result<VolumeRelation, CommandError> {
        let start_inside = self.contains(start);
        let end_inside = self.contains(end);
        if start_inside && end_inside {
            return Ok(VolumeRelation::INSIDE);
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
        Ok(VolumeRelation {
            window: false,
            crossing: start_inside || end_inside || self.contains(closest),
        })
    }

    fn classify_polyline(self, points: &[Point3]) -> Result<VolumeRelation, CommandError> {
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
        Ok(VolumeRelation {
            window: false,
            crossing,
        })
    }

    fn classify_circle(self, circle: Circle3) -> Result<VolumeRelation, CommandError> {
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
        Ok(VolumeRelation {
            window: height.hypot(planar + circle.radius()) <= self.radius,
            crossing: height.hypot((planar - circle.radius()).abs()) <= self.radius,
        })
    }

    fn classify_mesh(
        self,
        mesh: &viboceros_geometry::TriangleMesh,
    ) -> Result<VolumeRelation, CommandError> {
        let vertices = self.classify_points(mesh.vertices().iter().copied())?;
        if vertices.window || vertices.crossing {
            return Ok(vertices);
        }
        for index in 0..mesh.faces().len() {
            if self.contains(mesh.closest_point_on_face(index, self.center)?) {
                return Ok(VolumeRelation {
                    window: false,
                    crossing: true,
                });
            }
        }
        Ok(VolumeRelation::OUTSIDE)
    }
}

#[derive(Clone, Copy)]
struct SelectionBox {
    frame: Frame3,
    intervals: [[Real; 2]; 3],
}

impl SelectionBox {
    fn contains_local(self, point: [Real; 3]) -> bool {
        point
            .into_iter()
            .zip(self.intervals)
            .all(|(value, [low, high])| low <= value && value <= high)
    }

    fn contains(self, point: Point3) -> Result<bool, CommandError> {
        Ok(self.contains_local(self.frame.coordinates_of(point)?))
    }

    fn classify_bounds(self, bounds: BoundingBox3) -> Option<VolumeRelation> {
        let min = bounds.min().to_array();
        let max = bounds.max().to_array();
        let mut local_min = [Real::INFINITY; 3];
        let mut local_max = [Real::NEG_INFINITY; 3];
        for x in [min[0], max[0]] {
            for y in [min[1], max[1]] {
                for z in [min[2], max[2]] {
                    let point = Point3::try_new(x, y, z).ok()?;
                    let local = self.frame.coordinates_of(point).ok()?;
                    for axis in 0..3 {
                        local_min[axis] = local_min[axis].min(local[axis]);
                        local_max[axis] = local_max[axis].max(local[axis]);
                    }
                }
            }
        }
        if (0..3).any(|axis| {
            local_min[axis] > self.intervals[axis][1] || local_max[axis] < self.intervals[axis][0]
        }) {
            Some(VolumeRelation::OUTSIDE)
        } else if (0..3).all(|axis| {
            local_min[axis] >= self.intervals[axis][0] && local_max[axis] <= self.intervals[axis][1]
        }) {
            Some(VolumeRelation::INSIDE)
        } else {
            None
        }
    }

    fn classify(
        self,
        geometry: &Geometry,
        tolerance: Tolerance,
    ) -> Result<VolumeRelation, CommandError> {
        if !matches!(geometry, Geometry::PointCloud(_))
            && let Some(relation) = self.classify_bounds(geometry.bounds())
        {
            return Ok(relation);
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
            Geometry::Line(line) => self.classify_segment(line.start(), line.end()),
            Geometry::Polyline(polyline) => self.classify_polyline(polyline.vertices()),
            Geometry::Circle(circle) => self.classify_oval(
                circle.center(),
                circle.point_at_angle(0.0)?,
                circle.point_at_angle(std::f64::consts::FRAC_PI_2)?,
            ),
            Geometry::Ellipse(ellipse) => self.classify_oval(
                ellipse.center(),
                ellipse.point_at_angle(0.0)?,
                ellipse.point_at_angle(std::f64::consts::FRAC_PI_2)?,
            ),
            Geometry::Arc(arc) => {
                let center = arc.center();
                let cosine_point =
                    center.translated(arc.x_axis().as_vector().scaled(arc.radius())?)?;
                let sine_point =
                    center.translated(arc.y_axis().as_vector().scaled(arc.radius())?)?;
                self.classify_oval_interval(center, cosine_point, sine_point, arc.sweep_radians())
            }
            Geometry::Mesh(mesh) => self.classify_mesh(mesh),
            Geometry::NurbsSurface(surface) => {
                self.classify_mesh(&surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)?)
            }
            Geometry::Brep(brep) => {
                self.classify_mesh(&brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)?)
            }
            Geometry::NurbsCurve(_) | Geometry::PolyCurve(_) => {
                let curve = geometry.curve_ref().expect("curve geometry");
                let points = curve.sample_equal_length_points(CURVE_SAMPLES, true, tolerance)?;
                self.classify_polyline(&points)
            }
        }
    }

    fn classify_points(
        self,
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

    fn classify_segment(self, start: Point3, end: Point3) -> Result<VolumeRelation, CommandError> {
        let a = self.frame.coordinates_of(start)?;
        let b = self.frame.coordinates_of(end)?;
        let window = self.contains_local(a) && self.contains_local(b);
        if window {
            return Ok(VolumeRelation::INSIDE);
        }
        let mut entering: Real = 0.0;
        let mut leaving: Real = 1.0;
        for axis in 0..3 {
            let [low, high] = self.intervals[axis];
            let direct = [b[axis] - a[axis], low - a[axis], high - a[axis]];
            let (start, direction, low, high) = if direct.iter().all(|value| value.is_finite()) {
                (a[axis], direct[0], low, high)
            } else {
                let scale = [a[axis], b[axis], low, high]
                    .into_iter()
                    .map(Real::abs)
                    .fold(0.0, Real::max);
                (
                    a[axis] / scale,
                    b[axis] / scale - a[axis] / scale,
                    low / scale,
                    high / scale,
                )
            };
            if direction == 0.0 {
                if start < low || start > high {
                    return Ok(VolumeRelation::OUTSIDE);
                }
                continue;
            }
            let first = (low - start) / direction;
            let second = (high - start) / direction;
            entering = entering.max(first.min(second));
            leaving = leaving.min(first.max(second));
            if entering > leaving {
                return Ok(VolumeRelation::OUTSIDE);
            }
        }
        Ok(VolumeRelation {
            window: false,
            crossing: entering <= leaving,
        })
    }

    fn classify_polyline(self, points: &[Point3]) -> Result<VolumeRelation, CommandError> {
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
        Ok(VolumeRelation {
            window: false,
            crossing,
        })
    }

    /// An oval's coordinates on each box axis have the form
    /// `center + cosine * cos(angle) + sine * sin(angle)`. Extrema prove window
    /// containment; the six box-plane crossings partition the parameter
    /// circle into intervals whose inside/outside status is constant.
    fn classify_oval(
        self,
        center: Point3,
        cosine_point: Point3,
        sine_point: Point3,
    ) -> Result<VolumeRelation, CommandError> {
        self.classify_oval_interval(center, cosine_point, sine_point, std::f64::consts::TAU)
    }

    fn classify_oval_interval(
        self,
        center: Point3,
        cosine_point: Point3,
        sine_point: Point3,
        end_angle: Real,
    ) -> Result<VolumeRelation, CommandError> {
        let center = self.frame.coordinates_of(center)?;
        let cosine_point = self.frame.coordinates_of(cosine_point)?;
        let sine_point = self.frame.coordinates_of(sine_point)?;
        let cosine: [Real; 3] = std::array::from_fn(|axis| cosine_point[axis] - center[axis]);
        let sine: [Real; 3] = std::array::from_fn(|axis| sine_point[axis] - center[axis]);
        let radii: [Real; 3] = std::array::from_fn(|axis| cosine[axis].hypot(sine[axis]));
        let scale = center
            .into_iter()
            .chain(cosine)
            .chain(sine)
            .chain(self.intervals.into_iter().flatten())
            .map(Real::abs)
            .fold(1.0, Real::max);
        let epsilon = 32.0 * Real::EPSILON * scale;
        if (0..3).any(|axis| {
            center[axis] - radii[axis] > self.intervals[axis][1] + epsilon
                || center[axis] + radii[axis] < self.intervals[axis][0] - epsilon
        }) {
            return Ok(VolumeRelation::OUTSIDE);
        }
        let inside = |angle: Real| {
            let (sin_angle, cos_angle) = angle.sin_cos();
            (0..3).all(|axis| {
                let coordinate =
                    cosine[axis].mul_add(cos_angle, sine[axis].mul_add(sin_angle, center[axis]));
                coordinate >= self.intervals[axis][0] - epsilon
                    && coordinate <= self.intervals[axis][1] + epsilon
            })
        };
        let mut extrema = vec![0.0, end_angle];
        for axis in 0..3 {
            if radii[axis] == 0.0 {
                continue;
            }
            let phase = sine[axis]
                .atan2(cosine[axis])
                .rem_euclid(std::f64::consts::PI);
            for angle in [phase, phase + std::f64::consts::PI] {
                if angle <= end_angle {
                    extrema.push(angle);
                }
            }
        }
        if extrema.iter().copied().all(inside) {
            return Ok(VolumeRelation::INSIDE);
        }
        let mut angles = vec![0.0, end_angle];
        for axis in 0..3 {
            let radius = radii[axis];
            if radius == 0.0 {
                continue;
            }
            let phase = sine[axis].atan2(cosine[axis]);
            for &bound in &self.intervals[axis] {
                let ratio = (bound - center[axis]) / radius;
                if !ratio.is_finite() || ratio.abs() > 1.0 + 32.0 * Real::EPSILON {
                    continue;
                }
                let spread = ratio.clamp(-1.0, 1.0).acos();
                for angle in [phase - spread, phase + spread] {
                    let angle = angle.rem_euclid(std::f64::consts::TAU);
                    if angle <= end_angle {
                        angles.push(angle);
                    }
                }
            }
        }
        angles.sort_by(Real::total_cmp);
        let crossing = angles.iter().copied().any(inside)
            || angles
                .windows(2)
                .any(|pair| inside(pair[0].midpoint(pair[1])));
        Ok(VolumeRelation {
            window: false,
            crossing,
        })
    }

    fn classify_mesh(
        self,
        mesh: &viboceros_geometry::TriangleMesh,
    ) -> Result<VolumeRelation, CommandError> {
        let vertices = self.classify_points(mesh.vertices().iter().copied())?;
        if vertices.window || vertices.crossing {
            return Ok(vertices);
        }
        for index in 0..mesh.triangles().len() {
            let Some(points) = mesh.triangle_points(index) else {
                continue;
            };
            let triangle = [
                self.frame.coordinates_of(points[0])?,
                self.frame.coordinates_of(points[1])?,
                self.frame.coordinates_of(points[2])?,
            ];
            if self.triangle_crosses(triangle) {
                return Ok(VolumeRelation {
                    window: false,
                    crossing: true,
                });
            }
        }
        Ok(VolumeRelation::OUTSIDE)
    }

    fn triangle_crosses(self, triangle: [[Real; 3]; 3]) -> bool {
        let mut polygon = triangle.to_vec();
        for axis in 0..3 {
            for (bound, lower) in [
                (self.intervals[axis][0], true),
                (self.intervals[axis][1], false),
            ] {
                if polygon.is_empty() {
                    return false;
                }
                let mut clipped = Vec::new();
                let mut previous = *polygon.last().unwrap();
                let mut previous_inside = if lower {
                    previous[axis] >= bound
                } else {
                    previous[axis] <= bound
                };
                for &current in &polygon {
                    let current_inside = if lower {
                        current[axis] >= bound
                    } else {
                        current[axis] <= bound
                    };
                    if previous_inside != current_inside {
                        let t = (bound - previous[axis]) / (current[axis] - previous[axis]);
                        let mut intersection = [0.0; 3];
                        for coordinate in 0..3 {
                            intersection[coordinate] = previous[coordinate]
                                + t * (current[coordinate] - previous[coordinate]);
                        }
                        intersection[axis] = bound;
                        clipped.push(intersection);
                    }
                    if current_inside {
                        clipped.push(current);
                    }
                    previous = current;
                    previous_inside = current_inside;
                }
                polygon = clipped;
            }
        }
        !polygon.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{
        CircularArc3, Ellipse3, LineSegment, PointCloud3, TriangleMesh, UnitVector3, Vector3,
    };

    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn sel_box_finds_ovals_inside_a_gap_between_display_samples() {
        let frame = CommandContext::default().construction_plane;
        let theta: Real = 0.017;
        let (sin_theta, cos_theta) = theta.sin_cos();
        let x_axis = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        let y_axis = UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap();
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let circle =
            Circle3::try_from_frame(p(0.0, 0.0, 0.0), 1.0, x_axis, normal, Tolerance::DEFAULT)
                .unwrap();
        let ellipse = Ellipse3::try_new(
            p(0.0, 0.0, 0.0),
            2.0,
            1.0,
            x_axis,
            y_axis,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (geometry, x) in [
            (Geometry::Circle(circle), cos_theta),
            (Geometry::Ellipse(ellipse), 2.0 * cos_theta),
        ] {
            let region = SelectionBox {
                frame,
                intervals: [
                    [x - 2e-5, x + 2e-5],
                    [sin_theta - 2e-5, sin_theta + 2e-5],
                    [-0.1, 0.1],
                ],
            };
            assert!(
                region
                    .classify(&geometry, Tolerance::DEFAULT)
                    .unwrap()
                    .crossing,
                "{geometry:?}"
            );
            let points = geometry
                .curve_ref()
                .unwrap()
                .sample_equal_length_points(CURVE_SAMPLES, true, Tolerance::DEFAULT)
                .unwrap();
            assert!(
                !region.classify_polyline(&points).unwrap().crossing,
                "the previous chord approximation should miss this curve"
            );
            let outside = SelectionBox {
                intervals: [
                    [x + 1e-4, x + 2e-4],
                    [sin_theta - 2e-5, sin_theta + 2e-5],
                    [-0.1, 0.1],
                ],
                ..region
            };
            assert!(
                !outside
                    .classify(&geometry, Tolerance::DEFAULT)
                    .unwrap()
                    .crossing
            );
        }
        let arc = Geometry::Arc(CircularArc3::try_from_circle_sweep(circle, 0.1).unwrap());
        let arc_box = SelectionBox {
            frame,
            intervals: [[0.99, 1.01], [-0.01, 0.11], [-0.1, 0.1]],
        };
        assert!(arc_box.classify(&arc, Tolerance::DEFAULT).unwrap().window);
        let opposite_box = SelectionBox {
            intervals: [[-1.01, -0.99], [-0.01, 0.11], [-0.1, 0.1]],
            ..arc_box
        };
        assert!(
            !opposite_box
                .classify(&arc, Tolerance::DEFAULT)
                .unwrap()
                .crossing
        );
        let rotated = SelectionBox {
            frame: Frame3::try_from_directions(
                p(0.0, 0.0, 0.0),
                Vector3::try_new(1.0, 1.0, 0.0).unwrap(),
                Vector3::try_new(-1.0, 1.0, 0.0).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
            intervals: [[-1.1, 1.1], [-1.1, 1.1], [-0.1, 0.1]],
        };
        assert!(
            rotated
                .classify(&Geometry::Circle(circle), Tolerance::DEFAULT)
                .unwrap()
                .window
        );
    }

    #[test]
    fn sel_box_classifies_points_segments_and_faces_in_all_modes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let inside = document
            .add_geometry(Geometry::Point(p(1.0, 1.0, 1.0)))
            .unwrap();
        let outside = document
            .add_geometry(Geometry::Point(p(3.0, 1.0, 1.0)))
            .unwrap();
        let crossing = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(-1.0, 1.0, 1.0), p(3.0, 1.0, 1.0), Tolerance::DEFAULT)
                    .unwrap(),
            ))
            .unwrap();
        let face = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![p(-5.0, -5.0, 1.0), p(5.0, -5.0, 1.0), p(0.0, 5.0, 1.0)],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![p(1.0, 1.0, 1.0)])
                    .unwrap()
                    .with_hidden(vec![true])
                    .unwrap(),
            ))
            .unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        let undo = document.undo_label().map(str::to_owned);
        for (mode, expected) in [
            ("Window", vec![inside]),
            ("Crossing", vec![inside, crossing, face]),
            ("InvertWindow", vec![outside]),
            ("InvertCrossing", vec![outside, crossing, face]),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("SelBox 0,0,0 2,2,0 2 SelectionMode={mode}"),
                )
                .unwrap();
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                expected.into_iter().collect(),
                "{mode}",
            );
        }
        registry
            .execute(&mut document, "SelBox 0,0,2 2,2,2 -2")
            .unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([inside, crossing, face])
        );
        registry
            .execute(
                &mut document,
                "SelBox 0,0,0 2,2,0 2 SelectionMode=InvertCrossing",
            )
            .unwrap();
        for input in [
            "SelBox 0,0,0 2,2,0",
            "SelBox 0,0,0 2,2,0 0",
            "SelBox 0,0,0 0,2,0 2",
            "SelBox 0,0,0 2,2,0 2 SelectionMode=Nope",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                BTreeSet::from([outside, crossing, face]),
                "{input}",
            );
        }
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        assert_eq!(document.undo_label(), undo.as_deref());
    }

    #[test]
    fn sel_box_uses_active_construction_plane_and_height_point() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let inside = document
            .add_geometry(Geometry::Point(p(1.0, 1.0, 1.0)))
            .unwrap();
        document
            .add_geometry(Geometry::Point(p(-1.0, 1.0, 1.0)))
            .unwrap();
        let context = CommandContext {
            construction_plane: Frame3::try_from_directions(
                p(0.0, 0.0, 0.0),
                Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
                Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        };
        registry
            .execute_in_context(
                &mut document,
                "SelBox 0,0,0 0,2,2 2,0,0 SelectionMode=Window",
                context,
            )
            .unwrap();
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![inside]
        );
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
            VolumeRelation {
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
            VolumeRelation {
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
            VolumeRelation::OUTSIDE
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
