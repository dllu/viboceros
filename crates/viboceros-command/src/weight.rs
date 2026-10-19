//! Absolute rational weights use selected control points and one atomic edit.
use super::*;
use viboceros_document::{ControlPointId, GeometrySnapshot};

pub const USAGE: &str = "Weight positive-weight";

pub fn parse(arguments: &[&str]) -> Result<Option<Real>, CommandError> {
    match arguments {
        [] => Ok(None),
        [value] => {
            let value = parse_finite_real(value)?;
            if value <= 0. {
                return Err(CommandError::Usage(USAGE));
            }
            Ok(Some(value))
        }
        _ => Err(CommandError::Usage(USAGE)),
    }
}

#[derive(Clone, Debug)]
pub struct Prepared {
    points: Vec<ControlPointId>,
    selection: Vec<ControlPointId>,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    default: Real,
    varying: bool,
}
impl Prepared {
    pub fn capture(doc: &Document) -> Result<Self, CommandError> {
        let mut nets = BTreeMap::new();
        let physical = doc
            .selected_control_points()
            .map(|(p, _)| {
                let net = nets
                    .entry(p.object)
                    .or_insert_with(|| doc.control_point_weights(p.object));
                (p, net.as_ref().and_then(|w| w.get(p.index)).copied())
            })
            .collect::<Vec<_>>();
        let controls = physical
            .iter()
            .filter_map(|(p, w)| w.map(|w| (*p, w)))
            .collect::<Vec<_>>();
        if controls.is_empty() {
            return Err(CommandError::Usage(
                "Weight requires selected curve or surface control points",
            ));
        }
        let first = physical[0].1;
        let varying = physical.iter().any(|(_, w)| *w != first);
        let owners = controls
            .iter()
            .map(|(p, _)| p.object)
            .collect::<BTreeSet<_>>();
        let sources = doc
            .objects()
            .filter(|o| owners.contains(&o.id()))
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect();
        Ok(Self {
            selection: physical.into_iter().map(|(p, _)| p).collect(),
            points: controls.into_iter().map(|(p, _)| p).collect(),
            sources,
            default: if varying { 1. } else { first.unwrap() },
            varying,
        })
    }
    pub fn default_weight(&self) -> Real {
        self.default
    }
    pub fn weights_vary(&self) -> bool {
        self.varying
    }
    pub fn points(&self) -> &[ControlPointId] {
        &self.points
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.sources.iter().all(|(id, g)| {
            doc.object(*id)
                .is_some_and(|o| o.geometry_snapshot().shares_storage_with(g))
        }) && doc
            .selected_control_points()
            .map(|(p, _)| p)
            .eq(self.selection.iter().copied())
            && doc
                .selected_weight_control_points()
                .map(|(p, _)| p)
                .eq(self.points.iter().copied())
    }
    pub fn apply(&self, doc: &mut Document, weight: Real) -> Result<usize, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::Usage(
                "Weight sources changed; start the command again",
            ));
        }
        doc.set_control_point_weights(self.points.iter().copied(), weight)
            .map_err(Into::into)
    }
    pub fn preview(&self, doc: &Document, weight: Real) -> Result<Document, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::Usage(
                "Weight sources changed; start the command again",
            ));
        }
        let mut scene = doc.clone();
        scene.clear_history()?;
        self.apply(&mut scene, weight)?;
        Ok(scene)
    }
}

pub(super) struct WeightCommand;
impl Command for WeightCommand {
    fn name(&self) -> &'static str {
        "Weight"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(args)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Weight",
            filter: ObjectSelectionFilter::Grips,
            options: Vec::new(),
            menus: Vec::new(),
            choices: Vec::new(),
            workflow: ObjectSelectionWorkflow::QuestionAfterSelection {
                message: "New control point weight",
                escape_answer: None,
            },
        }))
    }
    fn object_selection_confirmation(
        &self,
        doc: &Document,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Prepared::capture(doc)?;
        self.object_selection_prompt(args)
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let prepared = Prepared::capture(doc)?;
        let weight = parse(args)?.ok_or(CommandError::Usage(USAGE))?;
        let updated = prepared.apply(doc, weight)?;
        Ok(if updated == 0 {
            "Control point weights unchanged".into()
        } else {
            format!("Updated control point weights on {updated} object(s)")
        })
    }
}
