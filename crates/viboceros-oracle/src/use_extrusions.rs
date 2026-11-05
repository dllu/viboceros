//! Application preference and its selection/redo witnesses.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub initial: String,
    pub answer: String,
}
pub fn snapshot(doc: &Document, commands: &CommandRegistry) -> Value {
    let points=doc.objects().filter_map(|object| {
        let Geometry::Point(point)=object.geometry() else {return None};
        Some(json!({"point":point.to_array(),"selected":if doc.selected_object_ids().any(|id|id==object.id()){2}else{0}}))
    }).collect::<Vec<_>>();
    json!({"use_extrusions":commands.use_extrusions(),"points":points})
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1
        || !["Extrusion", "Polysurface"].contains(&f.initial.as_str())
        || !["Extrusion", "Polysurface", "Enter", "Cancel"].contains(&f.answer.as_str())
    {
        return Err(ProbeError::FixtureInvariant(
            "fixed application preference required",
        ));
    }
    let mut doc = Document::new(tolerance);
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, &format!("UseExtrusions {}", f.initial))?;
    commands.execute(&mut doc, "Point 1,2,3")?;
    commands.execute(&mut doc, "SelAll")?;
    doc.clear_history()?;
    commands.execute(&mut doc, "Point 10,20,30")?;
    commands.execute(&mut doc, "Undo")?;
    let before = snapshot(&doc, &commands);
    if ["Extrusion", "Polysurface"].contains(&f.answer.as_str()) {
        commands.execute(&mut doc, &format!("UseExtrusions {}", f.answer))?;
    } else if f.answer == "Enter" {
        commands.execute(&mut doc, "UseExtrusions")?;
    }
    let after = snapshot(&doc, &commands);
    commands.execute(&mut doc, "Redo")?;
    Ok((
        json!({"before":before,"after":after,"redo":snapshot(&doc,&commands),"success":f.answer!="Cancel","redone":true}),
        0,
    ))
}
