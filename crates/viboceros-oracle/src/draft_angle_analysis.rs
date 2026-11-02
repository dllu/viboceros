//! Native analysis recipes; display geometry remains outside model history.
use super::*;
use viboceros_geometry::Real;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: String,
    pub preselect: bool,
    pub range: [Real; 2],
    pub up: [Real; 3],
    pub wires: bool,
    pub steps: Vec<String>,
}
impl Fixture {
    pub fn source(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        if self.source == "mesh" || self.source == "panels" {
            let mut points = Vec::new();
            let mut triangles = Vec::new();
            for (i, angle) in [-90_f64, -45., -10., -5., -2.5, 0., 2.5, 5., 10., 45., 90.]
                .into_iter()
                .enumerate()
            {
                let a = angle.to_radians();
                let x = i as Real * 3.;
                let start = points.len() as u32;
                points.extend([
                    Point3::try_new(x, 0., 0.)?,
                    Point3::try_new(x, 1., 0.)?,
                    Point3::try_new(x - a.sin(), 0., a.cos())?,
                ]);
                triangles.push([start, start + 1, start + 2]);
            }
            let mesh = TriangleMesh::try_new(points, triangles, tolerance)?;
            return Ok(if self.source == "mesh" {
                Geometry::Mesh(mesh)
            } else {
                Geometry::Brep(Brep::try_from_mesh(&mesh, false, tolerance)?)
            });
        }
        direction_analysis::Fixture {
            source: self.source.clone(),
            preselect: self.preselect,
            steps: vec![],
            companion: false,
            picks: Default::default(),
        }
        .source(tolerance)
    }
    pub fn start_command(&self) -> String {
        format!(
            "DraftAngleAnalysis Add Range={},{} Up={},{},{} ShowIsoCurves={}",
            self.range[0],
            self.range[1],
            self.up[0],
            self.up[1],
            self.up[2],
            if self.wires { "Yes" } else { "No" }
        )
    }
}

pub fn snapshot(doc: &Document, r: &CommandRegistry, id: ObjectId) -> Value {
    doc.object(id).map_or(json!({"exists":false,"selected":false,"hidden":false,"analysis":false}),|obj| {
        json!({"exists":true,"selected":doc.is_selected(id),"hidden":!obj.attributes().is_visible(),"analysis":r.object_has_draft_angle_analysis(id)})
    })
}
pub fn settings(doc: &Document, r: &CommandRegistry) -> Value {
    r.draft_angle_analysis_view(doc).map_or(
        Value::Null,
        |v| json!({"range":v.options.range,"up":v.options.up,"wires":v.options.show_isocurves}),
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
        || f.steps.iter().any(|s| {
            ![
                "DraftAngleAnalysis",
                "DraftAngleAnalysisOff",
                "Hide",
                "Show",
                "Delete",
                "Undo",
                "Redo",
                "SelNone",
            ]
            .contains(&s.as_str())
        })
    {
        return Err(ProbeError::FixtureInvariant("invalid draft-angle recipe"));
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
            if step == "SelNone" {
                doc.clear_selection();
            } else {
                let postselected = doc.selected_object_count() == 0;
                if step == "Delete"
                    || (matches!(step.as_str(), "DraftAngleAnalysis" | "Hide") && postselected)
                {
                    doc.select_objects_direct([id], SelectionMode::Replace)?;
                }
                let command = if step == "DraftAngleAnalysis" {
                    f.start_command()
                } else {
                    step.clone()
                };
                let execute = match step.as_str() {
                    "Undo" => doc.can_undo(),
                    "Redo" => doc.can_redo(),
                    _ => true,
                };
                if execute {
                    if postselected {
                        r.execute_postselected(&mut doc, &command, Default::default())?;
                    } else {
                        r.execute(&mut doc, &command)?;
                    }
                }
            }
            if let Some(obj) = doc.object(id)
                && obj.geometry() != &source
            {
                return Err(ProbeError::FixtureInvariant("analysis changed geometry"));
            }
            states.push(snapshot(&doc, &r, id));
            options.push(settings(&doc, &r));
        }
        Ok(json!({"states":states,"settings":options}))
    })
}
