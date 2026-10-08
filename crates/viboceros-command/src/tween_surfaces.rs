//! Atomic surface tween construction; correspondence belongs to the command.
use super::*;
use viboceros_geometry::try_tween_nurbs_surfaces;

const USAGE: &str = "TweenSurfaces [NumberOfSurfaces=n] [MatchMethod=None] [OutputLayer=CurrentLayer|StartSrf|EndSrf] [Sources=a,b] [FlipStartU=Yes|No] [FlipStartV=Yes|No] [SwapStartUV=Yes|No] [FlipEndU=Yes|No] [FlipEndV=Yes|No] [SwapEndUV=Yes|No]";
pub(super) struct TweenSurfacesCommand;

#[derive(Clone, Copy)]
enum OutputLayer {
    Current,
    Start,
    End,
}
struct Options {
    number: usize,
    layer: OutputLayer,
    sources: Option<[ObjectId; 2]>,
    reverse: [[bool; 3]; 2],
}
impl Command for TweenSurfacesCommand {
    fn name(&self) -> &'static str {
        "TweenSurfaces"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(args)?;
        if options.sources.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Surfaces,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = parse(args)?;
        let ids = if let Some(ids) = options.sources {
            ids
        } else {
            let ids = selected_ids(document)?;
            ids.try_into().map_err(|_| CommandError::Usage(USAGE))?
        };
        if ids[0] == ids[1] {
            return Err(CommandError::Usage(USAGE));
        }
        let mut surfaces = Vec::new();
        let mut attributes = Vec::new();
        let mut groups = Vec::new();
        for (i, id) in ids.into_iter().enumerate() {
            let object = document.object(id).ok_or(CommandError::Usage(USAGE))?;
            if !document.is_object_selectable(id) {
                return Err(CommandError::Usage(USAGE));
            }
            let mut surface = match object.geometry() {
                Geometry::NurbsSurface(s) => s.clone(),
                Geometry::Brep(b) if b.faces().len() == 1 => b.faces()[0].surface().clone(),
                _ => return Err(CommandError::Usage(USAGE)),
            };
            attributes.push(object.attributes().clone());
            groups.push(object.group_ids().to_vec());
            if options.reverse[i][2] {
                surface = surface.try_swapped_uv()?;
            }
            if options.reverse[i][0] {
                surface = surface.try_reversed_u()?;
            }
            if options.reverse[i][1] {
                surface = surface.try_reversed_v()?;
            }
            surfaces.push(surface);
        }
        let outputs = try_tween_nurbs_surfaces(&surfaces[0], &surfaces[1], options.number)?;
        let (attrs, membership) = match options.layer {
            OutputLayer::Current => (
                ObjectAttributes::on_layer(document.current_layer_id()),
                vec![],
            ),
            OutputLayer::Start => (attributes[0].clone(), groups[0].clone()),
            OutputLayer::End => (attributes[1].clone(), groups[1].clone()),
        };
        let breps = outputs
            .iter()
            .map(|s| Brep::try_surface_face(s.clone(), document.tolerance()))
            .collect::<Result<Vec<_>, _>>()?;
        for b in breps {
            let id = document.add_geometry_with_attributes(Geometry::Brep(b), attrs.clone())?;
            document.set_object_group_memberships(id, membership.iter().copied())?;
        }
        document.clear_selection();
        Ok(format!("Created {} tween surface(s)", options.number))
    }
}
fn parse(args: &[&str]) -> Result<Options, CommandError> {
    let mut result = Options {
        number: 1,
        layer: OutputLayer::Current,
        sources: None,
        reverse: [[false; 3]; 2],
    };
    let mut seen = BTreeSet::new();
    for arg in args {
        let (name, value) = arg.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let name = name.trim_start_matches('_').to_ascii_lowercase();
        let value = value.trim_start_matches('_');
        if !seen.insert(name.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match name.as_str() {
            "numberofsurfaces" | "number" => {
                result.number = value.parse().map_err(|_| CommandError::Usage(USAGE))?
            }
            "matchmethod" if value.eq_ignore_ascii_case("None") => {}
            "outputlayer" => {
                result.layer = match value.to_ascii_lowercase().as_str() {
                    "currentlayer" | "current" => OutputLayer::Current,
                    "startsrf" => OutputLayer::Start,
                    "endsrf" => OutputLayer::End,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            }
            "sources" => {
                result.sources = Some(
                    value
                        .split(',')
                        .map(|s| {
                            s.parse::<ObjectId>()
                                .map_err(|_| CommandError::Usage(USAGE))
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .try_into()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                )
            }
            "flipstartu" | "flipstartv" | "swapstartuv" | "flipendu" | "flipendv" | "swapenduv" => {
                let source = usize::from(name.contains("end"));
                let axis = if name.starts_with("swap") {
                    2
                } else if name.ends_with('v') {
                    1
                } else {
                    0
                };
                result.reverse[source][axis] =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    if !(1..=viboceros_geometry::MAX_SURFACE_TWEEN_COUNT).contains(&result.number) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surface_tweens_preserve_sources_and_layer_attributes_with_atomic_history() {
        for layer in ["CurrentLayer", "StartSrf", "EndSrf"] {
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0")
                .unwrap();
            registry
                .execute(&mut doc, "SrfPt 0,0,4 4,0,4 4,6,4 0,6,4")
                .unwrap();
            let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
            let group = doc.add_group(None, ids.iter().copied()).unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            doc.clear_history().unwrap();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "TweenSurfaces Sources={},{} NumberOfSurfaces=2 OutputLayer={layer}",
                        ids[0], ids[1]
                    ),
                )
                .unwrap();
            assert_eq!(doc.objects().len(), 4);
            for (&id, object) in ids.iter().zip(&before) {
                assert_eq!(doc.object(id).unwrap(), object);
            }
            for (i, o) in doc.objects().skip(2).enumerate() {
                let Geometry::Brep(b) = o.geometry() else {
                    panic!()
                };
                let s = b.faces()[0].surface();
                let p = s
                    .evaluate(*s.domain_u().start(), *s.domain_v().start())
                    .unwrap();
                assert!((p.z() - 4. * (i + 1) as f64 / 3.).abs() < 1e-10);
                assert_eq!(
                    o.group_ids(),
                    if layer == "CurrentLayer" {
                        &[][..]
                    } else {
                        std::slice::from_ref(&group)
                    }
                );
            }
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().len(), 4);
        }
    }
    #[test]
    fn invalid_tween_options_and_sources_leave_geometry_and_history_unchanged() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 0,0,0").unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        for args in [
            "Number=0",
            "Number=4097",
            "Number=2 Number=3",
            "MatchMethod=Refit",
            "Sources=bad",
            "FlipEndU=Maybe",
        ] {
            assert!(
                registry
                    .execute(&mut doc, &format!("TweenSurfaces {args}"))
                    .is_err()
            );
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
        }
    }
}
