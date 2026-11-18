//! Polygon sizing, phase and winding for the four Pyramid base getters.
use super::{BaseMode, Kind};
use std::f64::consts::{PI, TAU};
use viboceros_geometry::{Frame3, GeometryError, Point3, Tolerance, UnitVector3, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Base {
    pub frame: Frame3,
    pub sides: usize,
    pub radius: f64,
    pub inner_radius: Option<f64>,
    height_normal: UnitVector3,
    mode: BaseMode,
    flipped: bool,
    star_roll: f64,
}

impl Base {
    pub(super) fn radial(
        plane: Frame3,
        center: Point3,
        point: Point3,
        sides: usize,
        mode: BaseMode,
        on_guide: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let point = if on_guide {
            let [x, y] = plane.projected_coordinates_of(point)?;
            plane.point_at([x, y, 0.])?
        } else {
            point
        };
        let direction = center.direction_to(point)?;
        let radial =
            Frame3::try_from_unit_x_and_normal(center, direction, plane.z_axis(), tolerance)?;
        let size = center.distance_to(point)?;
        let (frame, radius) = if mode == BaseMode::Circumscribed {
            (
                rotated_frame(radial, PI / sides as f64, tolerance)?,
                size / (PI / sides as f64).cos(),
            )
        } else {
            (radial, size)
        };
        Ok(Self {
            frame,
            sides,
            radius,
            inner_radius: None,
            height_normal: frame.z_axis(),
            mode,
            flipped: false,
            star_roll: 0.0,
        })
    }

    pub(super) fn edge(
        plane: Frame3,
        start: Point3,
        end: Point3,
        sides: usize,
        flipped: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let along = start.direction_to(end)?;
        let edge = Frame3::try_from_unit_x_and_normal(start, along, plane.z_axis(), tolerance)?;
        let half = start.distance_to(end)? * 0.5;
        let apothem = half / (PI / sides as f64).tan();
        let center = edge.point_at([half, if flipped { -apothem } else { apothem }, 0.])?;
        let normal = if flipped {
            edge.z_axis().opposite()
        } else {
            edge.z_axis()
        };
        let frame = Frame3::try_from_unit_x_and_normal(
            center,
            center.direction_to(end)?,
            normal,
            tolerance,
        )?;
        Ok(Self {
            frame,
            sides,
            radius: half.hypot(apothem),
            inner_radius: None,
            height_normal: edge.z_axis(),
            mode: BaseMode::Edge,
            flipped,
            star_roll: 0.0,
        })
    }

    pub fn height_normal(self) -> UnitVector3 {
        self.height_normal
    }

    pub(super) fn with_inner_radius(mut self, value: f64) -> Result<Self, GeometryError> {
        if !value.is_finite() || value == 0. {
            return Err(GeometryError::Degenerate {
                context: "Pyramid second star radius",
            });
        }
        self.inner_radius = Some(value.abs());
        Ok(self)
    }

    pub(super) fn observe_star_roll(&mut self, point: Point3) {
        if let Ok([x, y]) = self.frame.projected_coordinates_of(point)
            && (x != 0.0 || y != 0.0)
        {
            self.star_roll = -y.atan2(x);
        }
    }

    pub(super) fn automatic_inner_radius(self) -> f64 {
        let step = PI / self.sides as f64;
        let ratio = if self.sides < 5 {
            0.25
        } else {
            (2. * step).cos() / step.cos()
        };
        self.radius * ratio
    }

    pub(super) fn perpendicular_to_apex(
        mut self,
        apex: Point3,
        kind: Kind,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let origin = self.frame.origin();
        if apex == origin {
            return Ok(self);
        }
        let normal = origin.direction_to(apex)?.opposite();
        let basis = Frame3::try_from_unit_normal(origin, normal)?;
        let angle = match self.mode {
            BaseMode::Inscribed => 0.0,
            BaseMode::Circumscribed => PI / self.sides as f64,
            BaseMode::Edge if !self.flipped || kind == Kind::TruncatedPyramid => {
                TAU / self.sides as f64
            }
            BaseMode::Edge => 0.0,
            BaseMode::Star => self.star_roll,
        };
        self.frame = rotated_frame(basis, angle, tolerance)?;
        Ok(self)
    }

    pub(super) fn top_size_frame(
        self,
        center: Point3,
        tolerance: Tolerance,
    ) -> Result<Frame3, GeometryError> {
        let frame = self.frame.with_origin(center);
        match self.mode {
            BaseMode::Circumscribed | BaseMode::Edge => {
                rotated_frame(frame, -PI / self.sides as f64, tolerance)
            }
            BaseMode::Inscribed | BaseMode::Star => Ok(frame),
        }
    }

    pub(super) fn top_reference_size(self) -> f64 {
        match self.mode {
            BaseMode::Circumscribed | BaseMode::Edge => {
                self.radius * (PI / self.sides as f64).cos()
            }
            BaseMode::Inscribed | BaseMode::Star => self.radius,
        }
    }

    pub(super) fn scaled_at(self, center: Point3, scale: f64) -> Self {
        Self {
            frame: self.frame.with_origin(center),
            radius: self.radius * scale,
            inner_radius: self.inner_radius.map(|radius| radius * scale),
            ..self
        }
    }

    pub fn rim(self) -> Result<Vec<Point3>, GeometryError> {
        self.sampled_rim(self.sides)
    }

    pub fn preview_rim(self, side_limit: usize) -> Result<Vec<Point3>, GeometryError> {
        self.sampled_rim(self.sides.min(side_limit.max(3)))
    }

    fn sampled_rim(self, sampled_sides: usize) -> Result<Vec<Point3>, GeometryError> {
        let step = TAU / self.sides as f64;
        let mut points =
            Vec::with_capacity(sampled_sides * (1 + usize::from(self.inner_radius.is_some())));
        for sample in 0..sampled_sides {
            let angle = (sample * self.sides / sampled_sides) as f64 * step;
            points.push(self.rim_point(self.radius, angle)?);
            if let Some(radius) = self.inner_radius {
                points.push(self.rim_point(radius, angle + 0.5 * step)?);
            }
        }
        Ok(points)
    }

    fn rim_point(self, radius: f64, angle: f64) -> Result<Point3, GeometryError> {
        let (sin, cos) = angle.sin_cos();
        self.frame.point_at([radius * cos, radius * sin, 0.0])
    }
}

fn rotated_frame(frame: Frame3, angle: f64, tolerance: Tolerance) -> Result<Frame3, GeometryError> {
    if angle == 0. {
        return Ok(frame);
    }
    let (sin, cos) = angle.sin_cos();
    let x = frame.x_axis().as_vector().to_array();
    let y = frame.y_axis().as_vector().to_array();
    let direction = Vector3::try_from(std::array::from_fn(|i| cos.mul_add(x[i], sin * y[i])))?
        .normalized_nonzero()?;
    Frame3::try_from_unit_x_and_normal(frame.origin(), direction, frame.z_axis(), tolerance)
}
