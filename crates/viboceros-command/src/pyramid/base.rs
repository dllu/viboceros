//! Polygon sizing, phase and winding for the four Pyramid base getters.
use super::{BaseMode, Kind};
use crate::polygon::rim::{self, rotated_frame};
use std::f64::consts::{PI, TAU};
use viboceros_geometry::{Frame3, GeometryError, Point3, Tolerance, UnitVector3};

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
        let (frame, radius) = rim::radial(
            radial,
            size,
            sides,
            mode == BaseMode::Circumscribed,
            tolerance,
        )?;
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
        let (frame, radius) = rim::edge(
            edge,
            end,
            start.distance_to(end)?,
            sides,
            flipped,
            tolerance,
        )?;
        Ok(Self {
            frame,
            sides,
            radius,
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
        rim::automatic_inner_radius(self.radius, self.sides)
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
        self.preview_rim(self.sides)
    }

    pub fn preview_rim(self, side_limit: usize) -> Result<Vec<Point3>, GeometryError> {
        rim::sampled(
            self.frame,
            self.radius,
            self.inner_radius,
            self.sides,
            side_limit,
        )
    }
}
