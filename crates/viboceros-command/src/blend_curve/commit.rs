use super::*;
impl Input {
    pub fn commit(&self, doc: &mut Document) -> Result<ObjectId, CommandError> {
        if !self.ready() || !self.current(doc) {
            return Err(CommandError::Usage(
                "BlendCrv references changed; pick the curves again",
            ));
        }
        let curve = self.preview.as_ref().ok_or(CommandError::Usage(USAGE))?;
        if (self.preferences.trim || self.preferences.join)
            && self.guides.iter().any(|g| g.source.source().1.is_some())
        {
            return Err(CommandError::Usage(
                "BlendCrv surface edges currently require Trim=No and Join=No",
            ));
        }
        let mut replacements = vec![];
        let mut pieces = vec![];
        if self.preferences.trim || self.preferences.join {
            for (i, g) in self.guides.iter().enumerate() {
                let c = g.source.curve_ref();
                let retained_end = g.end ^ g.flip;
                let domain = if !self.preferences.trim {
                    c.domain()
                } else if retained_end {
                    *c.domain().start()..=g.parameter
                } else {
                    g.parameter..=*c.domain().end()
                };
                let changed = domain != c.domain();
                let source = c.to_owned();
                let piece = if changed {
                    crate::curve_segments::trimmed(&source, domain)?
                } else {
                    source
                };
                if changed {
                    replacements.push((g.source.object_id(), Geometry::from(piece.clone())));
                }
                let reverse = if i == 0 { !retained_end } else { retained_end };
                pieces.push(if reverse {
                    piece.reversed(self.tolerance)?
                } else {
                    piece
                });
            }
        }
        let output = if self.preferences.join {
            if self.guides[0].source.object_id() == self.guides[1].source.object_id() {
                return Err(CommandError::Usage(
                    "BlendCrv joined sources must be distinct",
                ));
            }
            let start = curve.evaluate(*curve.domain().start())?;
            let end = curve.evaluate(*curve.domain().end())?;
            if pieces[0].as_ref().end_point()?.distance_to(start)? > self.tolerance.absolute()
                || pieces[1].as_ref().start_point()?.distance_to(end)? > self.tolerance.absolute()
            {
                return Err(CommandError::Usage(
                    "Enable Trim to join interior blend contacts",
                ));
            }
            Some(viboceros_geometry::PolyCurve3::concatenate(&[
                pieces[0].to_polycurve()?,
                viboceros_geometry::Curve3::NurbsCurve(curve.clone()).to_polycurve()?,
                pieces[1].to_polycurve()?,
            ])?)
        } else {
            None
        };
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(selected)?;
        doc.clear_selection();
        if let Some(output) = output {
            let first = self.guides[0].source.object_id();
            doc.replace_object_geometries([(first, Geometry::PolyCurve(output))])?;
            doc.delete_objects([self.guides[1].source.object_id()])?;
            Ok(first)
        } else {
            if self.preferences.trim {
                doc.replace_object_geometries(replacements)?;
            }
            Ok(doc.add_geometry(Geometry::NurbsCurve(curve.clone()))?)
        }
    }
}
