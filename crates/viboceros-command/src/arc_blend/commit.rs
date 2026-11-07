use super::*;
impl Input {
    pub fn commit(&self, doc: &mut Document) -> Result<ObjectId, CommandError> {
        if !self.current(doc) || !self.ready() {
            return Err(CommandError::Usage(
                "ArcBlend references changed; pick the curves again",
            ));
        }
        let curve = self.curve.as_ref().ok_or(CommandError::Usage(USAGE))?;
        if self.trim && self.guides.iter().any(|g| g.source.source().1.is_some()) {
            return Err(CommandError::Usage(
                "ArcBlend trims ordinary curves; use Trim=No for surface edges",
            ));
        }
        let mut replacements = Vec::new();
        let mut pieces = Vec::new();
        if self.trim {
            for (i, g) in self.guides.iter().enumerate() {
                let c = doc
                    .object(g.source.object_id())
                    .and_then(|o| o.geometry().curve_ref())
                    .ok_or(CommandError::Usage(USAGE))?;
                let source = c.to_owned();
                let domain = if g.end {
                    *c.domain().start()..=g.parameter
                } else {
                    g.parameter..=*c.domain().end()
                };
                let changed = domain != c.domain();
                let piece = if !changed {
                    source
                } else {
                    source.try_trimmed(domain)?
                };
                if changed {
                    replacements.push((g.source.object_id(), Geometry::from(piece.clone())));
                }
                let reverse = if i == 0 { !g.end } else { g.end };
                pieces.push(if reverse {
                    piece.reversed(self.tolerance)?
                } else {
                    piece
                });
            }
        }
        let output = if self.trim && self.join {
            if self.guides[0].source.object_id() == self.guides[1].source.object_id() {
                return Err(CommandError::Usage(
                    "ArcBlend joined self-blends require distinct trimmed source portions",
                ));
            }
            Geometry::PolyCurve(PolyCurve3::concatenate(&[
                pieces[0].to_polycurve()?,
                curve.clone(),
                pieces[1].to_polycurve()?,
            ])?)
        } else {
            Geometry::PolyCurve(curve.clone())
        };
        let selected = doc.selected_objects().map(|o| o.id()).collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(selected)?;
        doc.clear_selection();
        if self.trim && self.join {
            doc.delete_objects(self.guides.iter().map(|g| g.source.object_id()))?;
        } else if self.trim {
            doc.replace_object_geometries(replacements)?;
        }
        Ok(doc.add_geometry(output)?)
    }
}
