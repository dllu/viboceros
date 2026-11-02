//! Portable replay of native curvature analysis membership and settings.
use super::*;
use viboceros_geometry::{MeshFace, SubD};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: String,
    pub style: String,
    pub range: [f64; 2],
    pub preselect: bool,
    pub steps: Vec<String>,
}
impl Fixture {
    pub fn source(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        if self.source.starts_with("subd_") {
            let (points, faces) = match self.source.as_str() {
                "subd_plane" => (
                    vec![[0., 0., 0.], [3., 0., 0.], [3., 3., 0.], [0., 3., 0.]],
                    vec![[0, 1, 2, 3]],
                ),
                "subd_box" => (
                    vec![
                        [-2., -2., -2.],
                        [2., -2., -2.],
                        [2., 2., -2.],
                        [-2., 2., -2.],
                        [-2., -2., 2.],
                        [2., -2., 2.],
                        [2., 2., 2.],
                        [-2., 2., 2.],
                    ],
                    vec![
                        [0, 3, 2, 1],
                        [4, 5, 6, 7],
                        [0, 1, 5, 4],
                        [1, 2, 6, 5],
                        [2, 3, 7, 6],
                        [3, 0, 4, 7],
                    ],
                ),
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "unknown curvature SubD source",
                    ));
                }
            };
            let mesh = TriangleMesh::try_new_faces(
                points
                    .into_iter()
                    .map(Point3::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
                faces.into_iter().map(MeshFace::Quad).collect(),
                tolerance,
            )?;
            return Ok(Geometry::SubD(SubD::try_from_mesh(
                &mesh,
                Default::default(),
                tolerance,
            )?));
        }
        Ok(Geometry::Brep(
            brep_direction::Fixture {
                source: self.source.clone(),
                edit: "flip".into(),
                preselect: true,
                faces: None,
                sdk: false,
                sequence: None,
                cancel: false,
                pair: false,
            }
            .source(tolerance)?,
        ))
    }
    pub fn start_command(&self) -> String {
        format!(
            "CurvatureAnalysis Add Style={} Range={},{}",
            self.style, self.range[0], self.range[1]
        )
    }
}

pub fn snapshot(doc: &Document, r: &CommandRegistry, id: ObjectId) -> Value {
    doc.object(id).map_or(json!({"exists":false,"selected":false,"hidden":false,"analysis":false,"draft":false}),|o| {
        json!({"exists":true,"selected":doc.is_selected(id),"hidden":!o.attributes().is_visible(),"analysis":r.object_has_curvature_analysis(id),"draft":r.object_has_draft_angle_analysis(id)})
    })
}
pub fn settings(doc: &Document, r: &CommandRegistry) -> Value {
    r.curvature_analysis_view(doc).map_or(
        Value::Null,
        |v| json!({"style":v.options.style.name(),"ranges":v.options.ranges}),
    )
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1
        || f.steps.is_empty()
        || f.steps.len() > 24
        || !["Gaussian", "Mean", "MinRadius", "MaxRadius"].contains(&f.style.as_str())
        || f.range.iter().any(|v| !v.is_finite())
        || f.steps.iter().any(|s| {
            ![
                "CurvatureAnalysis",
                "CurvatureAnalysisOff",
                "DraftAngleAnalysis",
                "DraftAngleAnalysisOff",
                "Hide",
                "Show",
                "Delete",
                "Undo",
                "Redo",
                "MaxRange",
            ]
            .contains(&s.as_str())
        })
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid curvature analysis recipe",
        ));
    }
    let source = f.source(tolerance)?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let r = CommandRegistry::with_builtins();
        let id = doc.add_geometry(source.clone())?;
        if f.preselect {
            doc.select_objects_direct([id], SelectionMode::Replace)?;
        }
        doc.clear_history()?;
        let mut states = vec![snapshot(&doc, &r, id)];
        let mut options = vec![settings(&doc, &r)];
        for step in &f.steps {
            let postselected = doc.selected_object_count() == 0;
            if step == "Delete"
                || (matches!(
                    step.as_str(),
                    "CurvatureAnalysis" | "DraftAngleAnalysis" | "Hide"
                ) && postselected)
            {
                doc.select_objects_direct([id], SelectionMode::Replace)?;
            }
            let execute = match step.as_str() {
                "Undo" => doc.can_undo(),
                "Redo" => doc.can_redo(),
                _ => true,
            };
            if execute {
                let command = if step == "CurvatureAnalysis" {
                    f.start_command()
                } else if step == "MaxRange" {
                    "CurvatureAnalysis MaxRange".into()
                } else {
                    step.clone()
                };
                if postselected {
                    r.execute_postselected(&mut doc, &command, Default::default())?;
                } else {
                    r.execute(&mut doc, &command)?;
                }
            }
            if let Some(o) = doc.object(id)
                && o.geometry() != &source
            {
                return Err(ProbeError::FixtureInvariant("analysis mutated source"));
            }
            states.push(snapshot(&doc, &r, id));
            options.push(settings(&doc, &r));
        }
        Ok(json!({"states":states,"settings":options}))
    })
}
