//! Face-constrained point input shares the signed-distance offset producer.
use super::*;
impl Plan {
    pub fn prepare_through_point(
        doc: &Document,
        source: Source,
        target: Target,
        point: Point3,
        options: Options,
    ) -> Result<Self, CommandError> {
        Self::prepare_optional_through_point(doc, source, target, point, options)?
            .ok_or_else(|| GeometryError::OffsetThroughPointNoSolution.into())
    }
    // An exactly on-source point is skipped by the native Select getter.
    fn prepare_optional_through_point(
        doc: &Document,
        source: Source,
        target: Target,
        point: Point3,
        options: Options,
    ) -> Result<Option<Self>, CommandError> {
        options.validate()?;
        if !source.current(doc) || !target.current(doc) {
            return Err(CommandError::Usage(
                "Select the source curve and surface again",
            ));
        }
        let constrained = target.location(point, false)?.point;
        let inverse_source = if target.surface.plane(doc.tolerance())?.is_some() {
            let boundaries = Brep::try_surface_face((*target.surface).clone(), doc.tolerance())?
                .edges()
                .iter()
                .map(|e| e.curve().clone())
                .collect::<Vec<_>>();
            extend_planar_offset(source.curve().clone(), &boundaries, doc.tolerance())?
        } else {
            source.curve().clone()
        };
        let inverse = viboceros_geometry::surface_curve_offset_distance_through_point(
            &target.surface,
            &inverse_source,
            constrained,
            doc.tolerance(),
        )?;
        if inverse.signed_distance.abs() <= doc.tolerance().absolute() {
            return Ok(None);
        }
        let signed = inverse.signed_distance * if target.reversed { -1. } else { 1. };
        // ThroughPoint ignores Number, the initial picked side and Flip. Each
        // accepted point addresses the original source, never a prior copy.
        let mut plan = Plan::prepare(
            doc,
            source,
            target,
            None,
            Options {
                distance: signed,
                number: 1,
                flip: false,
            },
        )?;
        let mut minimum = Real::INFINITY;
        for curve in &plan.curves {
            let parameter = curve.closest_parameter(constrained, doc.tolerance())?;
            minimum = minimum.min(curve.evaluate(parameter)?.distance_to(constrained)?);
        }
        if minimum > doc.tolerance().absolute() {
            return Err(GeometryError::OffsetThroughPointNoSolution.into());
        }
        plan.options = options;
        plan.through = Some(point);
        Ok(Some(plan))
    }
}

/// A bounded, ordered set of point objects, fitted before any document edit.
pub struct PointSetPlan {
    source: Source,
    target: Target,
    points: Vec<(ObjectId, viboceros_document::GeometrySnapshot)>,
    pub curves: Vec<NurbsCurve>,
    pub skipped: usize,
}
impl PointSetPlan {
    pub fn prepare(
        doc: &Document,
        source: Source,
        target: Target,
        ids: &[ObjectId],
        options: Options,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        if ids.is_empty() || ids.len() > MAX_POINTS {
            return Err(CommandError::Usage(
                "Select between 1 and 1024 point objects",
            ));
        }
        let mut points = Vec::new();
        let mut seen = BTreeSet::new();
        // Validate every point before performing any expensive fitting.
        for &id in ids {
            if !seen.insert(id) {
                continue;
            }
            let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
            if !doc.is_object_selectable(id) || !matches!(object.geometry(), Geometry::Point(_)) {
                return Err(CommandError::Usage(
                    "Select a visible, unlocked point object",
                ));
            }
            points.push((id, object.geometry_snapshot().clone()));
        }
        let mut curves = Vec::new();
        let mut skipped = 0;
        for (id, _) in &points {
            let Geometry::Point(point) = doc.object(*id).unwrap().geometry() else {
                unreachable!()
            };
            if let Some(plan) = Plan::prepare_optional_through_point(
                doc,
                source.clone(),
                target.clone(),
                *point,
                options,
            )? {
                if curves.len() + plan.curves.len() > MAX_OUTPUTS {
                    return Err(CommandError::Usage("Too many surface offset curves"));
                }
                curves.extend(plan.curves);
            } else {
                skipped += 1;
            }
        }
        Ok(Self {
            source,
            target,
            points,
            curves,
            skipped,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.source.current(doc)
            && self.target.current(doc)
            && self.points.iter().all(|(id, snapshot)| {
                doc.is_object_selectable(*id)
                    && doc
                        .object(*id)
                        .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
            })
    }
    pub fn commit(
        &self,
        doc: &mut Document,
        group: &mut viboceros_document::HistoryGroup,
    ) -> Result<String, CommandError> {
        let no_output = self.curves.is_empty();
        if no_output {
            if !doc.history_group_is_current(group) {
                return Err(DocumentError::HistoryGroupStale.into());
            }
            // An ordinary no-output transaction folds transient pick-release
            // markers into existing history without creating an Undo step or
            // clearing Redo. A group checkpoint would record a selection-only step.
            doc.begin_transaction("OffsetCrvOnSrf")?;
        } else {
            doc.begin_group_transaction(group)?;
        }
        match self.apply(doc) {
            Ok(message) => {
                if no_output {
                    doc.commit_transaction()?;
                } else {
                    doc.commit_group_transaction(group)?;
                }
                Ok(message)
            }
            Err(error) => {
                doc.rollback_transaction()?;
                Err(error)
            }
        }
    }
    pub(super) fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "Source, surface or point objects changed; select them again",
            ));
        }
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(selected)?;
        doc.clear_selection();
        for curve in &self.curves {
            doc.add_geometry(Geometry::NurbsCurve(curve.clone()))?;
        }
        Ok(format!(
            "Created {} surface offset curve(s); skipped {} on-source point(s)",
            self.curves.len(),
            self.skipped
        ))
    }
}
