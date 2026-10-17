//! Flow uses the existing shared display cages and prepared guide mapping.
use super::display_cache::DisplayCache;
use super::morph_preview::{MorphPreviewCache, MorphPreviewImage, MorphPreviewKey};
use super::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use viboceros_command::surface_flow::Options;
use viboceros_geometry::{AffineTransform3, GeometryError, SurfaceFlowPointMorph};
#[derive(Clone)]
pub(crate) struct Key {
    morph: Arc<SurfaceFlowPointMorph>,
    options: Options,
}
impl PartialEq for Key {
    fn eq(&self, rhs: &Self) -> bool {
        Arc::ptr_eq(&self.morph, &rhs.morph)
            && self.options.rigid == rhs.options.rigid
            && self.options.rigid_groups == rhs.options.rigid_groups
    }
}
impl MorphPreviewKey for Key {
    type Morph = SurfaceFlowPointMorph;
    fn morph(&self, _: Tolerance) -> Result<Self::Morph, GeometryError> {
        Ok((*self.morph).clone())
    }
    fn rigid(&self) -> bool {
        self.options.rigid
    }
    fn rigid_groups(&self) -> bool {
        self.options.rigid_groups
    }
    fn rigid_bounds_tolerance(&self, _: Tolerance) -> Tolerance {
        Tolerance::NUMERICAL_VALIDATION
    }
    fn preserve(&self) -> bool {
        self.options.preserve_structure
    }
    fn rigid_transform(
        &self,
        morph: &Self::Morph,
        center: Point3,
    ) -> Result<AffineTransform3, GeometryError> {
        morph.rigid_transform(center)
    }
}
pub(crate) type SurfaceFlowPreviewCache = MorphPreviewCache<Key>;
#[derive(Clone, Copy)]
pub(crate) struct SurfaceFlowPreview<'a> {
    pub sources: &'a [ObjectId],
    pub grips: &'a [viboceros_document::ControlPointId],
    pub morph: &'a Arc<SurfaceFlowPointMorph>,
    pub options: Options,
    pub cache: &'a RefCell<SurfaceFlowPreviewCache>,
}
impl std::fmt::Debug for SurfaceFlowPreview<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceFlowPreview")
            .field("sources", &self.sources)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
impl SurfaceFlowPreview<'_> {
    pub(super) fn resolve(
        self,
        doc: &Document,
        display: &RefCell<DisplayCache>,
    ) -> Option<Rc<MorphPreviewImage>> {
        self.cache
            .borrow_mut()
            .get_with_grips(
                self.sources,
                self.grips,
                Key {
                    morph: self.morph.clone(),
                    options: self.options,
                },
                doc,
                display,
            )
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::{ControlPointId, SelectionMode};
    #[test]
    fn surface_flow_preview_shares_readonly_geometry_and_matches_group_and_grip_edits() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../tools/rhino_oracle/fixtures/surface_flow_command.json"
        ))
        .unwrap();
        for name in [
            "owner-nurbs-1-1000",
            "owner-surface-0-1001",
            "owner-mesh-0-1010",
        ] {
            let f: viboceros_oracle::SurfaceFlowFixture = serde_json::from_value(
                q["operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| o["id"] == name)
                    .unwrap()
                    .clone(),
            )
            .unwrap();
            let mut d = Document::default();
            let guides = f
                .surfaces(d.tolerance())
                .unwrap()
                .map(|s| d.add_geometry(Geometry::NurbsSurface(s)).unwrap());
            let source = d
                .add_geometry(f.source_geometry(d.tolerance()).unwrap())
                .unwrap();
            let peer = d
                .add_geometry(Geometry::Point(Point3::try_new(6., 4., 1.).unwrap()))
                .unwrap();
            d.add_group(None, [source, peer]).unwrap();
            if f.edit {
                d.enable_edit_points([source]).unwrap();
            } else {
                d.enable_control_points([source]).unwrap();
            }
            let grips = f
                .selected
                .iter()
                .map(|&index| ControlPointId {
                    object: source,
                    index,
                })
                .collect::<Vec<_>>();
            d.select_control_points(grips.iter().copied(), SelectionMode::Add)
                .unwrap();
            d.clear_history().unwrap();
            let before = format!("{d:?}");
            let options = Options {
                copy: false,
                rigid: f.options.rigid,
                rigid_groups: true,
                preserve_structure: f.options.preserve_structure,
                auto_adjust: false,
                ..Default::default()
            };
            let morph = Arc::new(
                viboceros_command::surface_flow::prepare(
                    &d,
                    viboceros_command::surface_flow::Reference {
                        id: guides[0],
                        face: None,
                    },
                    viboceros_command::surface_flow::Reference {
                        id: guides[1],
                        face: None,
                    },
                    options,
                )
                .unwrap(),
            );
            let display = RefCell::new(DisplayCache::default());
            let cache = RefCell::new(SurfaceFlowPreviewCache::default());
            let ids = [peer];
            let preview = SurfaceFlowPreview {
                sources: &ids,
                grips: &grips,
                morph: &morph,
                options,
                cache: &cache,
            };
            let image = preview.resolve(&d, &display).unwrap();
            assert!(Rc::ptr_eq(&image, &preview.resolve(&d, &display).unwrap()));
            let copied = SurfaceFlowPreview {
                options: Options {
                    copy: true,
                    ..options
                },
                ..preview
            };
            assert!(Rc::ptr_eq(&image, &copied.resolve(&d, &display).unwrap()));
            let selected = f.selected.iter().copied().collect();
            let mode = if f.edit {
                viboceros_document::GripMode::Edit
            } else {
                viboceros_document::GripMode::Control
            };
            let expected = d
                .object(source)
                .unwrap()
                .geometry()
                .with_morphed_grips_in_mode(&selected, &*morph, d.tolerance(), mode)
                .unwrap();
            assert_eq!(&*image.objects[&source].geometry.geometry, &expected);
            for (g, p) in image.grips.iter() {
                assert_eq!(*p, expected.grip_locations(mode).unwrap()[g.index]);
            }
            let staged = viboceros_command::surface_flow::deformed_with_grips(
                &d, &ids, &grips, &morph, options,
            )
            .unwrap();
            let object = &image.objects[&peer];
            let actual = if let Some(transform) = object.transform {
                object
                    .geometry
                    .geometry
                    .transformed(transform, d.tolerance())
                    .unwrap()
            } else {
                (*object.geometry.geometry).clone()
            };
            assert_eq!(actual, staged[0].1);
            assert_eq!(format!("{d:?}"), before);
            d.disable_control_points();
            assert!(preview.resolve(&d, &display).is_none());
        }
    }
}
