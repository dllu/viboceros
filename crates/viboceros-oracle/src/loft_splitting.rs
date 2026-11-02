//! Actual Loft option combinations and model undo states.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: Vec<NurbsCurveDefinition>,
    pub style: String,
    pub closed: bool,
    pub crease_splitting: bool,
    pub split_at_tangents: bool,
    pub preselect: bool,
    #[serde(default)]
    pub natural: bool,
    #[serde(default)]
    pub inspection: bool,
}
impl Fixture {
    pub fn sources(&self) -> Result<Vec<Geometry>, ProbeError> {
        self.curves
            .iter()
            .map(|c| Ok(Geometry::NurbsCurve(nurbs_curve_from_definition(c)?)))
            .collect()
    }
    pub fn command(&self) -> String {
        format!(
            "Loft Type={} Closed={} SplitAtTangents={}{}",
            self.style,
            if self.closed { "Yes" } else { "No" },
            if self.split_at_tangents { "Yes" } else { "No" },
            if self.natural { " Natural" } else { "" }
        )
    }
    pub fn crease_command(&self) -> &'static str {
        if self.crease_splitting {
            "CreaseSplitting Enable"
        } else {
            "CreaseSplitting Disable"
        }
    }
}

/// Product tensors sampled in their own charts and original source identity.
/// Native helper selection and attributes remain outside this model comparison.
pub fn snapshot(
    doc: &Document,
    ids: &[ObjectId],
    originals: &[Geometry],
) -> Result<Value, ProbeError> {
    if ids.len() != originals.len() {
        return Err(ProbeError::FixtureInvariant(
            "Loft source identities differ",
        ));
    }
    let source_indices = ids
        .iter()
        .enumerate()
        .map(|(index, &id)| (id, index))
        .collect::<std::collections::HashMap<_, _>>();
    let mut records = Vec::new();
    for obj in doc.objects() {
        if let Some(&i) = source_indices.get(&obj.id()) {
            records.push(json!({"source":i,"unchanged":obj.geometry()==&originals[i]}));
            continue;
        }
        let Geometry::Brep(b) = obj.geometry() else {
            return Err(ProbeError::FixtureInvariant("Loft result must be a B-rep"));
        };
        let mut faces = Vec::new();
        for face in b.faces() {
            let s = face.surface();
            let u = s.domain_u();
            let v = s.domain_v();
            let mut samples = Vec::new();
            for i in 0..=4 {
                for j in 0..=4 {
                    samples.push(
                        s.evaluate(
                            s.parameter_at_u(i as f64 / 4.)?,
                            s.parameter_at_v(j as f64 / 4.)?,
                        )?
                        .to_array(),
                    );
                }
            }
            faces.push(json!({"domain_u":[u.start(),u.end()],"domain_v":[v.start(),v.end()],"reversed":face.is_reversed(),"samples":samples}));
        }
        faces.sort_by(|a, b| {
            for key in ["domain_v", "domain_u"] {
                for i in 0..2 {
                    let order = a[key][i]
                        .as_f64()
                        .unwrap()
                        .total_cmp(&b[key][i].as_f64().unwrap());
                    if !order.is_eq() {
                        return order;
                    }
                }
            }
            std::cmp::Ordering::Equal
        });
        let bounds = obj.geometry().tight_bounds(doc.tolerance())?;
        records.push(
            json!({"source":null,"is_solid":b.is_solid(),"face_count":b.faces().len(),
            "edge_count":b.edges().len(),"vertex_count":b.vertices().len(),
            "bounds":[bounds.min().to_array(),bounds.max().to_array()],"faces":faces}),
        );
    }
    Ok(json!(records))
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 || f.inspection {
        return Err(ProbeError::FixtureInvariant(
            "Loft command replay requires one iteration and portable diagnostics",
        ));
    }
    let originals = f.sources()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let ids = originals
            .iter()
            .map(|g| doc.add_geometry(g.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, f.crease_command())?;
        registry.execute(&mut doc, &f.command())?;
        let after = snapshot(&doc, &ids, &originals)?;
        registry.execute(&mut doc, "Undo")?;
        let undo = snapshot(&doc, &ids, &originals)?;
        registry.execute(&mut doc, "Redo")?;
        let redo = snapshot(&doc, &ids, &originals)?;
        Ok(json!({"after":after,"undo":undo,"redo":redo}))
    })
}
