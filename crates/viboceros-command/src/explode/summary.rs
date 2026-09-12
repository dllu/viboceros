use super::{CommandError, MAX_SPAN_OUTPUT_OBJECTS, too_many_span_outputs};

#[derive(Clone, Copy)]
pub(super) enum PartKind {
    Polycurve,
    Polyline,
    PointCloud,
    Polysurface,
    Mesh,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct ExplodeSummary {
    sources: [usize; 5],
    outputs: [usize; 5],
    total: usize,
}

impl ExplodeSummary {
    pub(super) fn check_add(&self, count: usize) -> Result<usize, CommandError> {
        self.total
            .checked_add(count)
            .filter(|total| *total <= MAX_SPAN_OUTPUT_OBJECTS)
            .ok_or_else(|| too_many_span_outputs("Explode"))
    }

    pub(super) fn record(&mut self, kind: PartKind, count: usize) -> Result<(), CommandError> {
        let total = self.check_add(count)?;
        let index = kind as usize;
        self.sources[index] += 1;
        self.outputs[index] += count;
        self.total = total;
        Ok(())
    }

    pub(super) fn message(&self, unchanged: usize) -> String {
        let labels = [
            ("polycurve(s)", "curve(s)"),
            ("polyline(s)", "line(s)"),
            ("point cloud(s)", "point(s)"),
            ("polysurface(s)", "surface(s)"),
            ("mesh(es)", "part(s)"),
        ];
        let mut summaries = Vec::with_capacity(labels.len());
        for (index, (source, output)) in labels.into_iter().enumerate() {
            if self.sources[index] > 0 {
                summaries.push(format!(
                    "{} {source} into {} {output}",
                    self.sources[index], self.outputs[index]
                ));
            }
        }
        let last = summaries
            .pop()
            .expect("at least one selected object was exploded");
        let summary = if summaries.is_empty() {
            last
        } else {
            format!("{} and {last}", summaries.join(", "))
        };
        format!("Exploded {summary}; {unchanged} object(s) unchanged")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_keeps_category_order_and_accumulates_repeated_sources() {
        let mut summary = ExplodeSummary::default();
        for (kind, count) in [
            (PartKind::Mesh, 2),
            (PartKind::PointCloud, 3),
            (PartKind::Polyline, 4),
            (PartKind::Mesh, 5),
            (PartKind::Polycurve, 6),
            (PartKind::Polysurface, 7),
        ] {
            summary.record(kind, count).unwrap();
        }
        assert_eq!(
            summary.message(8),
            "Exploded 1 polycurve(s) into 6 curve(s), 1 polyline(s) into 4 line(s), 1 point cloud(s) into 3 point(s), 1 polysurface(s) into 7 surface(s) and 2 mesh(es) into 7 part(s); 8 object(s) unchanged"
        );
    }

    #[test]
    fn output_limit_and_overflow_rejections_do_not_change_counts() {
        let mut summary = ExplodeSummary::default();
        summary
            .record(PartKind::Mesh, MAX_SPAN_OUTPUT_OBJECTS - 1)
            .unwrap();
        let before = format!("{summary:?}");
        assert_eq!(summary.check_add(1).unwrap(), MAX_SPAN_OUTPUT_OBJECTS);
        assert!(summary.check_add(2).is_err());
        assert_eq!(format!("{summary:?}"), before);
        summary.record(PartKind::Mesh, 1).unwrap();
        assert_eq!(
            summary.message(0),
            format!(
                "Exploded 2 mesh(es) into {MAX_SPAN_OUTPUT_OBJECTS} part(s); 0 object(s) unchanged"
            )
        );
        for count in [1, usize::MAX] {
            let before = format!("{summary:?}");
            assert!(summary.record(PartKind::Polyline, count).is_err());
            assert_eq!(format!("{summary:?}"), before);
        }
    }
}
