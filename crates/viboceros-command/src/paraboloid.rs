//! Point stages and exact geometry shared by Paraboloid interaction and scripts.
mod command;
#[cfg(test)]
mod tests;

use crate::{
    parabolic_axis::{Construction, Dimensions},
    translation::DestinationConstraint,
};
pub(super) use command::ParaboloidCommand;
use viboceros_drafting::PointConstraintState;
use viboceros_geometry::{
    Brep, Frame3, GeometryError, NurbsSurface, Point3, Real, Tolerance, UnitVector3,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub mark_focus: bool,
    pub solid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    First(Construction),
    Second(Construction, Point3),
    End(Construction, Point3, Point3),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    stage: Stage,
    preferences: Preferences,
    tolerance: Tolerance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    dimensions: Dimensions,
    preferences: Preferences,
    tolerance: Tolerance,
}

impl Input {
    pub fn new(preferences: Preferences, tolerance: Tolerance) -> Self {
        Self {
            stage: Stage::First(Construction::Focus),
            preferences,
            tolerance,
        }
    }

    pub fn tolerance(self) -> Tolerance {
        self.tolerance
    }

    pub fn preferences(self) -> Preferences {
        self.preferences
    }

    pub fn anchor(self) -> Option<Point3> {
        self.markers()[0]
    }

    pub fn point_constraint(self, plane: Frame3) -> Option<PointConstraintState> {
        let state = PointConstraintState::new(self.anchor()?, plane);
        Some(
            self.rim_normal()
                .map_or(state, |normal| state.with_distance_plane(normal)),
        )
    }

    /// A free mouse pick uses the intersection of the rim plane and the
    /// active CPlane through the vertex. Parallel planes leave the azimuth free.
    pub fn mouse_line(self, plane: Frame3) -> Option<DestinationConstraint> {
        let direction = self
            .rim_normal()?
            .as_vector()
            .cross(plane.z_axis().as_vector())
            .ok()?;
        if direction.length().ok()? <= self.tolerance.angular() {
            return None;
        }
        Some(DestinationConstraint {
            anchor: self.anchor()?,
            direction: Some(direction.normalized_nonzero().ok()?),
            distance: None,
        })
    }

    fn rim_normal(self) -> Option<UnitVector3> {
        let Stage::End(Construction::VertexFocus, vertex, focus) = self.stage else {
            return None;
        };
        vertex.direction_to(focus).ok()
    }

    pub fn markers(self) -> [Option<Point3>; 2] {
        match self.stage {
            Stage::First(_) => [None, None],
            Stage::Second(_, first) => [Some(first), None],
            Stage::End(_, first, second) => [Some(first), Some(second)],
        }
    }

    pub fn prompt(self) -> String {
        let (point, mode) = match self.stage {
            Stage::First(Construction::Focus) => ("focus", "Vertex, "),
            Stage::First(_) => ("vertex", ""),
            Stage::Second(Construction::VertexFocus, _) => ("focus", "Direction, "),
            Stage::Second(_, _) => ("direction", ""),
            Stage::End(..) => ("end", ""),
        };
        let yes_no = |value| if value { "Yes" } else { "No" };
        format!(
            "Paraboloid {point} ({mode}MarkFocus={}, Solid={})",
            yes_no(self.preferences.mark_focus),
            yes_no(self.preferences.solid)
        )
    }

    pub fn option(&mut self, word: &str) -> Result<bool, GeometryError> {
        let word = word.trim_start_matches(['_', '-']);
        if let Stage::First(_) = self.stage {
            if word.eq_ignore_ascii_case("Focus") {
                return Ok(true);
            }
            if word.eq_ignore_ascii_case("Vertex") {
                self.stage = Stage::First(Construction::VertexFocus);
                return Ok(true);
            }
        }
        if let Stage::Second(Construction::VertexFocus, vertex) = self.stage
            && word.eq_ignore_ascii_case("Direction")
        {
            self.stage = Stage::Second(Construction::VertexDirection, vertex);
            return Ok(true);
        }
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(name, value)| (name, Some(value)));
        let setting = if name.eq_ignore_ascii_case("MarkFocus") {
            &mut self.preferences.mark_focus
        } else if name.eq_ignore_ascii_case("Solid") {
            &mut self.preferences.solid
        } else {
            return Ok(false);
        };
        *setting = match value {
            None => !*setting,
            Some(value) => crate::parse_yes_no(value).ok_or(GeometryError::Degenerate {
                context: "paraboloid option requires Yes or No",
            })?,
        };
        Ok(true)
    }

    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, GeometryError> {
        self.stage = match self.stage {
            Stage::First(construction) => Stage::Second(construction, point),
            Stage::Second(construction, first) => {
                first.vector_to(point)?.normalized(self.tolerance)?;
                Stage::End(construction, first, point)
            }
            Stage::End(construction, first, second) => {
                return self
                    .definition(construction, first, second, point)
                    .map(Some);
            }
        };
        Ok(None)
    }

    pub fn preview(self, point: Point3) -> Option<Definition> {
        let Stage::End(construction, first, second) = self.stage else {
            return None;
        };
        self.definition(construction, first, second, point).ok()
    }

    fn definition(
        self,
        construction: Construction,
        first: Point3,
        second: Point3,
        end: Point3,
    ) -> Result<Definition, GeometryError> {
        Ok(Definition {
            dimensions: construction.dimensions(first, second, end, self.tolerance)?,
            preferences: self.preferences,
            tolerance: self.tolerance,
        })
    }
}

impl Definition {
    pub fn focus_marker(self) -> Option<Point3> {
        self.preferences.mark_focus.then_some(self.dimensions.focus)
    }

    pub fn focal_distance(self) -> Real {
        self.dimensions.focal_distance
    }

    pub fn wall(self) -> Result<NurbsSurface, GeometryError> {
        let Dimensions {
            frame,
            radius,
            height,
            ..
        } = self.dimensions;
        NurbsSurface::try_paraboloid(frame, radius, height)
    }

    pub fn brep(self) -> Result<Brep, GeometryError> {
        let Dimensions {
            frame,
            radius,
            height,
            ..
        } = self.dimensions;
        Brep::try_paraboloid(
            frame,
            radius,
            height,
            self.preferences.solid,
            self.tolerance,
        )
    }
}
