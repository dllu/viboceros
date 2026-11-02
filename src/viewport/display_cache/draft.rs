//! Camera-independent color buffers shared by all four viewports.
use super::*;
use std::sync::Arc;
use viboceros_geometry::{CurvatureMesh, DraftAngleMapping};

pub(super) struct Drawing {
    mapping: DraftAngleMapping,
    source: Option<Arc<CurvatureMesh>>,
    colors: DraftColors,
}

impl DisplayGeometry {
    pub(crate) fn draft_colors(
        &self,
        source: Option<&Arc<CurvatureMesh>>,
        mapping: DraftAngleMapping,
    ) -> DraftColors {
        let mut cached = self.draft_colors.borrow_mut();
        if let Some(drawing) = cached.as_ref()
            && drawing.mapping == mapping
            && match (source, drawing.source.as_ref()) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
        {
            return drawing.colors.clone();
        }
        let colors: DraftColors = if let Some(source) = source {
            viboceros_command::analysis_mesh::draft_colors(source, mapping).into()
        } else {
            self.normals()
                .iter()
                .map(|normals| {
                    normals.map(|n| {
                        Vector3::try_new(n.x, n.y, n.z).map_or([0, 0, 0], |n| mapping.color(n))
                    })
                })
                .collect()
        };
        *cached = Some(Drawing {
            mapping,
            source: source.cloned(),
            colors: colors.clone(),
        });
        colors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_color_buffers_are_reused_and_settings_replace_them() {
        let mut doc = Document::default();
        let r = viboceros_command::CommandRegistry::with_builtins();
        r.execute(&mut doc, "SrfPt -2,-2,0 2,-2,0 2,2,0 -2,2,0")
            .unwrap();
        let mut cache = DisplayCache::default();
        let object = doc.objects().next().unwrap();
        let display = cache.get(object, doc.tolerance(), false);
        let mapping =
            DraftAngleMapping::try_new(Vector3::try_new(0., 0., 1.).unwrap(), [0., 5.]).unwrap();
        let first = display.draft_colors(None, mapping);
        for _ in 0..4 {
            assert!(Rc::ptr_eq(&first, &display.draft_colors(None, mapping)));
        }
        let reverse =
            DraftAngleMapping::try_new(Vector3::try_new(0., 0., -1.).unwrap(), [0., 5.]).unwrap();
        let changed = display.draft_colors(None, reverse);
        assert!(!Rc::ptr_eq(&first, &changed));
        assert!(first.iter().flatten().all(|c| *c == [0, 0, 255]));
        assert!(changed.iter().flatten().all(|c| *c == [255, 0, 0]));
    }
}
