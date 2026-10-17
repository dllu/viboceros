//! Immutable rail capture, shared command construction and script selectors.
use super::*;
use crate::curve_reference::CapturedCurve;
use std::sync::Arc;
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{CurveRef, CurveSample};

#[derive(Clone, Debug)]
pub(super) struct Rail {
    pub(super) captured: Arc<CapturedCurve>,
    pub(super) id: ObjectId,
    pub(super) edge: Option<usize>,
    pub(super) length: Real,
    pub(super) ends: [CurveSample; 2],
}
impl PartialEq for Rail {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.edge == other.edge
            && Arc::ptr_eq(&self.captured, &other.captured)
    }
}
impl Rail {
    fn capture(doc: &Document, id: ObjectId, edge: Option<usize>) -> Result<Self, CommandError> {
        let captured = Arc::new(CapturedCurve::capture(doc, id, edge)?);
        let curve = captured.curve_ref();
        let length = curve.length(doc.tolerance())?;
        if length <= doc.tolerance().absolute() {
            return Err(invalid("spiral rail is too short"));
        }
        let samples = curve.divide_by_count_samples(1, true, doc.tolerance())?;
        Ok(Self {
            captured,
            id,
            edge,
            length,
            ends: [samples[0], samples[1]],
        })
    }
    pub(super) fn curve(&self) -> CurveRef<'_> {
        self.captured.curve_ref()
    }
}
impl Input {
    pub fn capture_rail(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
    ) -> Result<(), CommandError> {
        if self.slot() != Slot::Rail {
            return Err(invalid("choose AroundCurve before selecting a rail"));
        }
        let rail = Rail::capture(doc, id, edge)?;
        let mut next = self.clone();
        next.start = Some(rail.ends[0].point());
        next.end = Some(rail.ends[1].point());
        next.rail = Some(rail);
        next.tolerance = doc.tolerance();
        next.update_advance()?;
        *self = next;
        Ok(())
    }
    pub fn rail_current(&self, doc: &Document) -> bool {
        self.rail.as_ref().is_none_or(|r| r.captured.current(doc))
    }
    pub fn rail_source(&self) -> Option<(ObjectId, Option<usize>)> {
        self.rail.as_ref().map(|r| (r.id, r.edge))
    }
    pub fn clear_rail(&mut self) {
        self.rail = None;
        self.start = None;
        self.end = None;
        self.radial = None;
        self.end_radius = None;
        self.locked_radius = None;
        self.locked_height = None;
        self.complete = false;
    }
}

/// The existing script's selected/named rail and explicit stable object/edge selectors.
pub(crate) fn run(
    doc: &mut Document,
    kind: Kind,
    arguments: &[&str],
) -> Result<String, CommandError> {
    let usage = if kind == Kind::Helix {
        crate::HELIX_USAGE
    } else {
        crate::SPIRAL_USAGE
    };
    let words = &arguments[1..];
    let count = words.iter().take_while(|w| !w.contains('=')).count();
    let radii = if kind == Kind::Spiral {
        crate::parse_spiral_radii(&words[..count], usage)?
    } else {
        let radius = match &words[..count] {
            [word] => crate::parse_inline_axial_radius(word)?,
            [x, y, z] => AxialPrimitiveRadius::Point(crate::parse_point(&[*x, *y, *z])?.0),
            _ => return Err(CommandError::Usage(usage)),
        };
        [radius, radius]
    };
    let AxialPrimitiveRadius::Point(radius_point) = radii[0] else {
        return Err(CommandError::Usage(usage));
    };
    let mut source = None;
    let mut density = None;
    let mut advance = Vec::new();
    for word in &words[count..] {
        let Some((key, value)) = word.split_once('=') else {
            return Err(CommandError::Usage(usage));
        };
        if ["PathName", "Curve", "Edge"]
            .iter()
            .any(|k| crate::option_name_eq(key, k))
        {
            if source.is_some() || value.is_empty() {
                return Err(CommandError::Usage(usage));
            }
            source = Some((key, value));
        } else if crate::option_name_eq(key, "PointsPerTurn")
            || crate::option_name_eq(key, "NumPointsPerTurn")
        {
            if density.is_some() {
                return Err(CommandError::Usage(usage));
            }
            density = Some(
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= viboceros_geometry::MIN_SWEPT_SPIRAL_POINTS_PER_TURN)
                    .ok_or(CommandError::Usage(usage))?,
            );
        } else {
            advance.push(*word);
        }
    }
    let (advance, reverse) = crate::parse_spiral_advance_options(&advance, usage)?;
    let (id, edge) = match source {
        Some((key, value)) if crate::option_name_eq(key, "Curve") => {
            (value.parse().map_err(|_| CommandError::Usage(usage))?, None)
        }
        Some((key, value)) if crate::option_name_eq(key, "Edge") => {
            let (id, index) =
                crate::curve_reference::parse_edge(value).ok_or(CommandError::Usage(usage))?;
            (id, Some(index))
        }
        Some((_, name)) => {
            let mut ids = doc
                .objects()
                .filter(|o| {
                    o.attributes()
                        .name()
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                })
                .map(|o| o.id());
            let id = ids
                .next()
                .ok_or_else(|| CommandError::SpiralPathNotFound(name.to_owned()))?;
            if ids.next().is_some() {
                return Err(CommandError::AmbiguousSpiralPath(name.to_owned()));
            }
            (id, None)
        }
        None => (
            doc.selected_object_ids()
                .last()
                .ok_or(CommandError::SpiralPathRequired)?,
            None,
        ),
    };
    let rail = Rail::capture(doc, id, edge)?;
    let curve = rail.curve();
    let start = crate::curve_radius_value(rail.ends[0], radii[0])?;
    let end = if kind == Kind::Helix {
        start
    } else {
        crate::curve_radius_value(rail.ends[1], radii[1])?
    };
    if start <= 0. || end < 0. {
        return Err(GeometryError::InvalidSpiralDimensions.into());
    }
    let turns = match advance {
        crate::SpiralAdvance::Turns(n) => n,
        crate::SpiralAdvance::Pitch(p) => rail.length / p,
    };
    let density = density.unwrap_or(viboceros_geometry::DEFAULT_SWEPT_SPIRAL_POINTS_PER_TURN);
    let output = NurbsCurve::try_swept_spiral(
        curve,
        radius_point,
        if reverse { -turns } else { turns },
        [start, end],
        density,
        doc.tolerance(),
    )?;
    let output_id = doc.add_geometry(Geometry::NurbsCurve(output))?;
    if edge.is_none() {
        doc.select_object(id, viboceros_document::SelectionMode::Add)?;
        doc.release_command_selection_on_history_replay([id])?;
    }
    Ok(format!(
        "Added NURBS {} {output_id} around curve {id} (radii {start:.6} to {end:.6}, turns {turns:.6}, {density} points per turn{})",
        kind.name().to_ascii_lowercase(),
        if reverse { ", reverse twist" } else { "" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandContext, CommandRegistry};
    use viboceros_geometry::{AffineTransform3, LineSegment, Vector3};
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn scene() -> (Document, ObjectId) {
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 10.), doc.tolerance()).unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        (doc, id)
    }
    #[test]
    fn around_rail_preview_and_committed_command_share_geometry_and_selection_replay() {
        let (mut doc, id) = scene();
        let source = doc.object(id).unwrap().geometry_snapshot().clone();
        let mut input = Input::new(
            Kind::Helix,
            Options::default(),
            CommandContext::default().construction_plane,
            doc.tolerance(),
        );
        input.option("AroundCurve").unwrap();
        assert_eq!(input.slot(), Slot::Rail);
        input.capture_rail(&doc, id, None).unwrap();
        input.option("Turns=1.25").unwrap();
        input.option("NumPointsPerTurn=8").unwrap();
        input.number(2.).unwrap();
        let before = input.clone();
        let preview = input.preview(p(0., 5., 0.)).unwrap();
        assert_eq!(input, before);
        assert_eq!(doc.objects().len(), 1);
        assert!(doc.undo_label().is_none());
        input.point(p(0., 5., 0.)).unwrap();
        assert_eq!(preview.control_points().len(), 13);
        let commands = CommandRegistry::with_builtins();
        commands
            .execute(&mut doc, &input.command().unwrap())
            .unwrap();
        let output = doc.objects().find(|o| o.id() != id).unwrap();
        let Geometry::NurbsCurve(c) = output.geometry() else {
            panic!()
        };
        assert_eq!(c, &preview);
        assert!(doc.is_selected(id));
        assert!(!doc.is_selected(output.id()));
        assert!(source.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
        assert_eq!(doc.undo().unwrap().as_deref(), Some("Helix"));
        assert_eq!(doc.objects().len(), 1);
        assert!(!doc.is_selected(id));
        assert_eq!(doc.redo().unwrap().as_deref(), Some("Helix"));
        assert!(!doc.is_selected(id));
        assert!(source.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    }
    #[test]
    fn around_rail_length_drives_pitch_and_bad_density_is_atomic() {
        let (doc, id) = scene();
        let mut input = Input::new(
            Kind::Spiral,
            Options::default(),
            CommandContext::default().construction_plane,
            doc.tolerance(),
        );
        input.option("AroundCurve").unwrap();
        input.numeric(Numeric::Pitch, 2.).unwrap();
        input.capture_rail(&doc, id, None).unwrap();
        assert_eq!(input.options().turns, 5.);
        let before = input.clone();
        for value in [0., 4., 5.5, Real::NAN, Real::INFINITY] {
            assert!(input.numeric(Numeric::PointsPerTurn, value).is_err());
            assert_eq!(input, before);
        }
        assert!(input.point(p(0., 0., 5.)).is_err());
        assert_eq!(input, before);
        input.point(p(1., 0., 0.)).unwrap();
        input.number(0.).unwrap();
        assert_eq!(input.curve().unwrap().control_points().len(), 63);
    }
    #[test]
    fn around_rail_capture_detects_geometry_and_selectability_changes() {
        let (mut doc, id) = scene();
        let mut input = Input::new(
            Kind::Helix,
            Options::default(),
            CommandContext::default().construction_plane,
            doc.tolerance(),
        );
        input.option("AroundCurve").unwrap();
        input.capture_rail(&doc, id, None).unwrap();
        let before = input.clone();
        assert!(input.rail_current(&doc));
        doc.set_objects_locked([id], true).unwrap();
        assert!(!input.rail_current(&doc));
        doc.set_objects_locked([id], false).unwrap();
        assert!(input.rail_current(&doc));
        doc.transform_objects(
            [id],
            AffineTransform3::from_translation(Vector3::try_new(5., 0., 0.).unwrap()),
        )
        .unwrap();
        assert!(!input.rail_current(&doc));
        assert_eq!(input, before);
        input.clear_rail();
        assert_eq!(input.slot(), Slot::Rail);
        input.capture_rail(&doc, id, None).unwrap();
        assert_eq!(input.anchor(), Some(p(5., 0., 0.)));
        assert!(input.rail_current(&doc));
    }
    #[test]
    fn around_script_accepts_named_object_and_edge_selectors_and_rejects_ambiguity() {
        let (mut doc, id) = scene();
        doc.set_object_names([(id, Some("Rail".into()))]).unwrap();
        let commands = CommandRegistry::with_builtins();
        commands
            .execute(
                &mut doc,
                "Helix AroundCurve 1,0,0 PathName=rail Turns=1 NumPointsPerTurn=12",
            )
            .unwrap();
        let source = doc.object(id).unwrap().geometry_snapshot().clone();
        let count = doc.objects().len();
        for command in [
            format!("Helix AroundCurve 1,0,0 Curve={id} PathName=Rail"),
            format!("Spiral AroundCurve 1,0,0 2 Curve={id} PointsPerTurn=4"),
            format!("Helix AroundCurve 1,0,0 Edge={id},0"),
        ] {
            assert!(commands.execute(&mut doc, &command).is_err());
            assert_eq!(doc.objects().len(), count);
            assert!(source.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
        }
    }
}
