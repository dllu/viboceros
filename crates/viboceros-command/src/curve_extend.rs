//! Shared length and boundary producers, plus captured interactive extensions.
use super::*;
use crate::curve_pair_edit::source_is_current;
use viboceros_document::{HistoryGroup, Object};
use viboceros_geometry::{
    CurveExtensionBoundary, CurveRef, LineSegment, ParameterSide, UnitVector3,
};

pub use viboceros_geometry::CurveExtensionStyle as Style;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Join {
    Merge,
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Options {
    pub style: Style,
    pub join: Join,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            style: Style::Natural,
            join: Join::Merge,
        }
    }
}

impl Options {
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), CommandError> {
        if option_name_eq(name, "Type") {
            self.style = [
                ("Natural", Style::Natural),
                ("Arc", Style::Arc),
                ("Line", Style::Line),
                ("Smooth", Style::Smooth),
            ]
            .into_iter()
            .find_map(|(name, style)| option_name_eq(value, name).then_some(style))
            .ok_or(CommandError::Usage(
                "Type expects Natural, Arc, Line, or Smooth",
            ))?;
        } else if option_name_eq(name, "Join") {
            self.join = [("Merge", Join::Merge), ("Yes", Join::Yes), ("No", Join::No)]
                .into_iter()
                .find_map(|(name, join)| option_name_eq(value, name).then_some(join))
                .ok_or(CommandError::Usage("Join expects Merge, Yes, or No"))?;
        } else {
            return Err(CommandError::Usage("Extend options are Type and Join"));
        }
        Ok(())
    }
}

pub(super) fn length_result(
    curve: &NurbsCurve,
    side: CurveExtensionSide,
    length: Real,
    options: Options,
    tolerance: Tolerance,
) -> Result<NurbsCurve, CommandError> {
    let joined = options.join == Join::Yes;
    Ok(match options.style {
        Style::Natural | Style::Smooth => extension_backend::curve_length(
            curve,
            side,
            length,
            tolerance,
            options.style == Style::Natural,
            joined,
        )?,
        Style::Line if joined => curve.try_joined_linearly_by_length(side, length, tolerance)?,
        Style::Line => curve.try_merged_linearly_by_length(side, length, tolerance)?,
        Style::Arc if joined => curve.try_joined_circularly_by_length(side, length, tolerance)?,
        Style::Arc => curve.try_merged_circularly_by_length(side, length, tolerance)?,
    })
}

pub(super) fn length_pieces(
    curve: &NurbsCurve,
    side: CurveExtensionSide,
    length: Real,
    style: Style,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    Ok(match style {
        Style::Natural | Style::Smooth => extension_backend::curve_pieces(
            curve,
            side,
            length,
            tolerance,
            style == Style::Natural,
        )?,
        Style::Arc => curve.try_separate_circular_extensions_by_length(side, length, tolerance)?,
        Style::Line => curve.try_separate_linear_extensions_by_length(side, length, tolerance)?,
    })
}

pub(super) fn boundary_geometry(
    geometry: &Geometry,
) -> Result<Option<CurveExtensionBoundary>, GeometryError> {
    Ok(match geometry {
        Geometry::NurbsSurface(surface) => Some(CurveExtensionBoundary::Surface(surface.clone())),
        Geometry::Brep(brep) => Some(CurveExtensionBoundary::Brep(brep.clone())),
        geometry => geometry
            .nurbs_curve_representation()?
            .map(CurveExtensionBoundary::Curve),
    })
}

#[derive(Clone)]
pub struct Boundary {
    object: Object,
    geometry: std::sync::Arc<CurveExtensionBoundary>,
}

impl Boundary {
    pub fn accepts(geometry: &Geometry) -> bool {
        geometry.curve_ref().is_some()
            || matches!(geometry, Geometry::NurbsSurface(_) | Geometry::Brep(_))
    }

    pub fn capture(document: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let object = visible_object(document, id)?;
        let geometry = boundary_geometry(object.geometry())?.ok_or(CommandError::Usage(
            "Select a curve, surface, or polysurface boundary",
        ))?;
        Ok(Self {
            object,
            geometry: geometry.into(),
        })
    }

    pub fn id(&self) -> ObjectId {
        self.object.id()
    }

    pub fn current(&self, document: &Document) -> bool {
        source_is_current(document, &self.object)
    }
}

#[derive(Clone)]
pub struct Source {
    object: Object,
    edge: Option<usize>,
    curve: std::sync::Arc<Curve3>,
    nurbs: std::sync::Arc<NurbsCurve>,
    at_start: bool,
    tolerance: Tolerance,
}

impl Source {
    pub fn capture(
        document: &Document,
        id: ObjectId,
        edge: Option<usize>,
        point: Option<Point3>,
    ) -> Result<Self, CommandError> {
        let object = visible_object(document, id)?;
        let tolerance = document.tolerance();
        let resolved = curve_reference::resolve(object.geometry(), edge, tolerance)
            .ok_or(CommandError::Usage("Select an open curve or surface edge"))?;
        let curve = resolved.curve();
        if curve.is_closed()? {
            return Err(CommandError::Usage("Select an open curve or surface edge"));
        }
        let at_start = if let Some(point) = point {
            curve.start_point()?.distance_to(point)? < curve.end_point()?.distance_to(point)?
        } else {
            true
        };
        let nurbs = curve.to_nurbs()?.into();
        let curve = curve.to_owned().into();
        Ok(Self {
            object,
            edge,
            curve,
            nurbs,
            at_start,
            tolerance,
        })
    }

    pub fn id(&self) -> ObjectId {
        self.object.id()
    }

    pub fn current(&self, document: &Document) -> bool {
        self.tolerance == document.tolerance() && source_is_current(document, &self.object)
    }

    pub fn endpoint(&self) -> Result<Point3, GeometryError> {
        if self.at_start {
            self.curve.as_ref().as_ref().start_point()
        } else {
            self.curve.as_ref().as_ref().end_point()
        }
    }

    pub fn tangent(&self) -> Result<UnitVector3, GeometryError> {
        let curve = self.curve.as_ref().as_ref();
        let (t, side) = if self.at_start {
            (*curve.domain().start(), ParameterSide::Right)
        } else {
            (*curve.domain().end(), ParameterSide::Left)
        };
        let tangent = curve.evaluate_with_tangent_on_side(t, side)?.tangent();
        Ok(if self.at_start {
            tangent.opposite()
        } else {
            tangent
        })
    }

    fn side(&self) -> CurveExtensionSide {
        if self.at_start {
            CurveExtensionSide::Start
        } else {
            CurveExtensionSide::End
        }
    }
}

fn visible_object(document: &Document, id: ObjectId) -> Result<Object, CommandError> {
    document
        .object(id)
        .filter(|_| document.is_object_selectable(id))
        .cloned()
        .ok_or_else(|| viboceros_document::DocumentError::ObjectNotFound(id).into())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Length(Real),
    Boundary,
    Projected(Point3),
    ToPoint(Point3),
    ArcCenter { center: Point3, end: Point3 },
    ArcCenterLength { center: Point3, length: Real },
}

pub struct Prepared {
    source: Source,
    curve: Geometry,
    replace: bool,
}

impl Prepared {
    pub fn geometry(&self) -> &Geometry {
        &self.curve
    }

    pub fn commit(
        self,
        document: &mut Document,
        group: &mut HistoryGroup,
    ) -> Result<ObjectId, CommandError> {
        if !self.source.current(document) {
            return Err(CommandError::Usage(
                "Extend source or document tolerance changed",
            ));
        }
        document.begin_group_transaction(group)?;
        let result = (|| -> Result<_, viboceros_document::DocumentError> {
            document.release_command_selection_on_history_replay(
                document.selected_object_ids().collect::<Vec<_>>(),
            )?;
            if self.replace {
                let id = self.source.id();
                document.replace_object_geometries([(id, self.curve)])?;
                Ok(id)
            } else {
                document.add_geometry_with_attributes(
                    self.curve,
                    self.source.object.attributes().clone(),
                )
            }
        })();
        match result {
            Ok(id) => {
                document.commit_group_transaction(group)?;
                document.clear_selection();
                Ok(id)
            }
            Err(error) => {
                document.rollback_transaction()?;
                Err(error.into())
            }
        }
    }
}

mod interactive;
pub use interactive::prepare;
