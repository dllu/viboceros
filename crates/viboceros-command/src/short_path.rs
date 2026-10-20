//! Immutable shortest-path plans shared by scripts and interactive previews.
use super::*;
use crate::orient_surface::Target;
use viboceros_geometry::{Point2, SurfaceShortPath, try_short_path_on_surface};

pub const USAGE: &str = "ShortPath start end [Surface=id] [Face=index]";
#[derive(Default)]
pub struct Input<'a> {
    pub points: Vec<&'a str>,
    pub surface: Option<ObjectId>,
    pub face: Option<usize>,
}
pub fn input<'a>(args: &[&'a str]) -> Result<Input<'a>, CommandError> {
    let mut result = Input::default();
    let mut seen = BTreeSet::new();
    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim_start_matches(['_', '-']).to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "surface" => {
                    result.surface = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                "face" => {
                    result.face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else {
            result.points.push(arg);
        }
    }
    Ok(result)
}
pub fn selected_target(doc: &Document) -> Option<ObjectId> {
    let ids = doc
        .selected_objects()
        .filter(|o| matches!(o.geometry(), Geometry::NurbsSurface(_) | Geometry::Brep(_)))
        .map(|o| o.id())
        .collect::<Vec<_>>();
    if let [id] = ids.as_slice() {
        Some(*id)
    } else {
        None
    }
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub target: Target,
    pub path: SurfaceShortPath,
}
impl Plan {
    pub fn prepare(
        doc: &Document,
        target: Target,
        start: Point3,
        end: Point3,
    ) -> Result<Self, CommandError> {
        if !target.current(doc) {
            return Err(CommandError::Usage("Select the ShortPath surface again"));
        }
        let a = target.location(start, false)?;
        let b = target.location(end, false)?;
        let path = try_short_path_on_surface(
            &target.surface,
            Point2::try_from(a.uv)?,
            Point2::try_from(b.uv)?,
            doc.tolerance(),
            morph_interpolation_solver(),
        )?;
        // Natural-surface paths must not silently cross a trimmed-out region.
        // Use the shared complete curve/face admission, including hole loops.
        if let Some(object) = doc.object(target.id)
            && let Geometry::Brep(brep) = object.geometry()
        {
            let face = &brep.faces()[target.face];
            if !face.is_untrimmed(doc.tolerance())? {
                let uv = target
                    .surface
                    .try_pullback_curve_certified(&path.curve, doc.tolerance())?;
                let events = viboceros_geometry::clip_surface_curve_to_face_with_operation(
                    &path.curve,
                    &uv,
                    face,
                    doc.tolerance(),
                    &mut curve_intersections::events,
                )?;
                let complete = matches!(events.as_slice(), [viboceros_geometry::SurfaceSurfaceIntersectionEvent::Curve(c)] if c.domain() == path.curve.domain());
                if !complete {
                    return Err(CommandError::Usage(
                        "ShortPath crosses a face trim; boundary detours are not supported yet",
                    ));
                }
            }
        }
        Ok(Self { target, path })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.target.current(doc)
    }
    pub fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage("Select the ShortPath surface again"));
        }
        doc.add_geometry(Geometry::NurbsCurve(self.path.curve.clone()))?;
        Ok(format!(
            "Created surface path of length {}",
            self.path.distance
        ))
    }
    pub fn commit(
        &self,
        doc: &mut Document,
        group: &mut viboceros_document::HistoryGroup,
    ) -> Result<String, CommandError> {
        doc.begin_group_transaction(group)?;
        match self.apply(doc) {
            Ok(message) => {
                doc.commit_group_transaction(group)?;
                Ok(message)
            }
            Err(error) => {
                doc.rollback_transaction()?;
                Err(error)
            }
        }
    }
}
pub struct ShortPathCommand;
impl Command for ShortPathCommand {
    fn name(&self) -> &'static str {
        "ShortPath"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = input(args)?;
        let (a, n) = parse_point(&input.points)?;
        let (b, m) = parse_point(&input.points[n..])?;
        if n + m != input.points.len() {
            return Err(CommandError::Usage(USAGE));
        }
        let id = input
            .surface
            .or_else(|| selected_target(doc))
            .ok_or(CommandError::Usage(USAGE))?;
        let target = Target::capture(doc, id, input.face)?;
        Plan::prepare(doc, target, a, b)?.apply(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn scene() -> (Document, ObjectId) {
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    p(-5., -5., 0.),
                    p(5., -5., 0.),
                    p(5., 5., 0.),
                    p(-5., 5., 0.),
                ])
                .unwrap(),
            ))
            .unwrap();
        (doc, id)
    }
    #[test]
    fn scripted_short_path_projects_points_and_groups_history() {
        let (mut doc, id) = scene();
        let registry = CommandRegistry::with_builtins();
        let source = doc.object(id).unwrap().geometry_snapshot().clone();
        registry
            .execute(&mut doc, &format!("ShortPath 0,0,9 3,4,7 Surface={id}"))
            .unwrap();
        assert_eq!(doc.objects().count(), 2);
        assert_eq!(doc.undo_label(), Some("ShortPath"));
        let c = doc
            .objects()
            .last()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!(c.start_point().unwrap().distance_to(p(0., 0., 0.)).unwrap() < 1e-8);
        assert!(c.end_point().unwrap().distance_to(p(3., 4., 0.)).unwrap() < 1e-8);
        assert!(source.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
        doc.undo().unwrap();
        assert_eq!(doc.objects().count(), 1);
        doc.redo().unwrap();
        assert_eq!(doc.objects().count(), 2);
    }
    #[test]
    fn invalid_short_path_and_stale_plans_leave_redo_untouched() {
        let (mut doc, id) = scene();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, &format!("ShortPath 0,0,0 1,1,0 Surface={id}"))
            .unwrap();
        doc.undo().unwrap();
        let before = format!("{doc:?}");
        for input in [
            format!("ShortPath 0,0,0 Surface={id}"),
            format!("ShortPath 0,0,0 1,1,0 Surface={id} Face=99"),
            format!("ShortPath 0,0,0 1,1,0 Surface={id} Surface={id}"),
        ] {
            assert!(registry.execute(&mut doc, &input).is_err());
            assert_eq!(format!("{doc:?}"), before);
        }
        let plan = Plan::prepare(
            &doc,
            Target::capture(&doc, id, None).unwrap(),
            p(0., 0., 0.),
            p(1., 1., 0.),
        )
        .unwrap();
        assert!(doc.set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-9).unwrap()));
        assert!(!plan.current(&doc));
        assert!(plan.apply(&mut doc).is_err());
        assert_eq!(doc.objects().count(), 1);
    }
    #[test]
    fn trimmed_face_holes_reject_crossing_paths_and_keep_complete_ones() {
        let (mut doc, id) = scene();
        let polygon = |radius: Real| {
            Polyline3::try_new(
                vec![
                    p(-radius, -radius, 0.),
                    p(radius, -radius, 0.),
                    p(radius, radius, 0.),
                    p(-radius, radius, 0.),
                    p(-radius, -radius, 0.),
                ],
                doc.tolerance(),
            )
            .unwrap()
            .to_nurbs()
            .unwrap()
        };
        let face = Brep::try_planar_face_with_holes(&polygon(5.), &[polygon(1.)], doc.tolerance())
            .unwrap();
        doc.replace_object_geometries([(id, Geometry::Brep(face))])
            .unwrap();
        let before = format!("{doc:?}");
        let registry = CommandRegistry::with_builtins();
        assert!(
            registry
                .execute(&mut doc, &format!("ShortPath -4,0,0 4,0,0 Surface={id}"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
        registry
            .execute(&mut doc, &format!("ShortPath -4,2,0 4,2,0 Surface={id}"))
            .unwrap();
        assert_eq!(doc.objects().count(), 2);
    }
}
