//! Application settings stored by eframe, separate from model undo history.

use viboceros_command::interface::{ViewportTabAlignment, ZoomScale};

use super::DEFAULT_ZOOM_SCALE;
use crate::viewport::ZoomExtentsBorders;

const ZOOM_SCALE_KEY: &str = "viboceros.view.zoom_scale.v1";
const PARALLEL_BORDER_KEY: &str = "viboceros.view.zoom_extents_parallel_border.v1";
const PERSPECTIVE_BORDER_KEY: &str = "viboceros.view.zoom_extents_perspective_border.v1";
const VIEWPORT_TABS_KEY: &str = "viboceros.view.viewport_tabs_visible.v1";
const VIEWPORT_TAB_ALIGNMENT_KEY: &str = "viboceros.view.viewport_tab_alignment.v1";

pub(super) fn load_viewport_tab_alignment(
    storage: Option<&dyn eframe::Storage>,
) -> ViewportTabAlignment {
    storage
        .and_then(|storage| storage.get_string(VIEWPORT_TAB_ALIGNMENT_KEY))
        .and_then(|value| ViewportTabAlignment::parse(&value))
        .unwrap_or_default()
}

pub(super) fn save_viewport_tab_alignment(
    storage: &mut dyn eframe::Storage,
    alignment: ViewportTabAlignment,
) {
    storage.set_string(VIEWPORT_TAB_ALIGNMENT_KEY, alignment.label().to_owned());
}

pub(super) fn load_viewport_tabs_visible(storage: Option<&dyn eframe::Storage>) -> bool {
    storage
        .and_then(|storage| storage.get_string(VIEWPORT_TABS_KEY))
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(true)
}

pub(super) fn save_viewport_tabs_visible(storage: &mut dyn eframe::Storage, visible: bool) {
    storage.set_string(VIEWPORT_TABS_KEY, visible.to_string());
}

fn load_positive_scale(storage: Option<&dyn eframe::Storage>, key: &str, default: f64) -> f64 {
    storage
        .and_then(|storage| storage.get_string(key))
        .filter(|value| value.len() <= 64)
        .and_then(|value| value.parse::<f64>().ok())
        .and_then(ZoomScale::try_new)
        .map_or(default, ZoomScale::value)
}

pub(super) fn load_zoom_scale(storage: Option<&dyn eframe::Storage>) -> f64 {
    load_positive_scale(storage, ZOOM_SCALE_KEY, DEFAULT_ZOOM_SCALE)
}

pub(super) fn save_zoom_scale(storage: &mut dyn eframe::Storage, scale: f64) {
    debug_assert!(ZoomScale::try_new(scale).is_some());
    storage.set_string(ZOOM_SCALE_KEY, scale.to_string());
}

pub(super) fn load_zoom_extents_borders(
    storage: Option<&dyn eframe::Storage>,
) -> ZoomExtentsBorders {
    let default = ZoomExtentsBorders::default();
    ZoomExtentsBorders {
        parallel: load_positive_scale(storage, PARALLEL_BORDER_KEY, default.parallel),
        perspective: load_positive_scale(storage, PERSPECTIVE_BORDER_KEY, default.perspective),
    }
}

pub(super) fn save_zoom_extents_borders(
    storage: &mut dyn eframe::Storage,
    borders: ZoomExtentsBorders,
) {
    debug_assert!(borders.valid());
    storage.set_string(PARALLEL_BORDER_KEY, borders.parallel.to_string());
    storage.set_string(PERSPECTIVE_BORDER_KEY, borders.perspective.to_string());
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::Storage;

    use super::*;

    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn zoom_scale_round_trips_and_invalid_storage_uses_default() {
        let mut storage = MemoryStorage::default();
        assert_eq!(load_zoom_scale(Some(&storage)), DEFAULT_ZOOM_SCALE);
        let mut app = super::super::tests::test_app();
        app.zoom_scale = 1.25;
        eframe::App::save(&mut app, &mut storage);
        assert_eq!(load_zoom_scale(Some(&storage)), 1.25);
        for value in ["0", "NaN", "Infinity", "5e-324", "invalid"] {
            storage.set_string(ZOOM_SCALE_KEY, value.into());
            assert_eq!(load_zoom_scale(Some(&storage)), DEFAULT_ZOOM_SCALE);
        }
        storage.set_string(ZOOM_SCALE_KEY, "1".repeat(65));
        assert_eq!(load_zoom_scale(Some(&storage)), DEFAULT_ZOOM_SCALE);
        assert_eq!(load_zoom_scale(None), DEFAULT_ZOOM_SCALE);
    }

    #[test]
    fn zoom_extents_borders_round_trip_and_fallback_independently() {
        let mut storage = MemoryStorage::default();
        let default = ZoomExtentsBorders::default();
        assert_eq!(load_zoom_extents_borders(Some(&storage)), default);
        let mut app = super::super::tests::test_app();
        app.zoom_extents_borders = ZoomExtentsBorders {
            parallel: 1.5,
            perspective: 0.8,
        };
        eframe::App::save(&mut app, &mut storage);
        assert_eq!(
            load_zoom_extents_borders(Some(&storage)),
            app.zoom_extents_borders
        );
        storage.set_string(PARALLEL_BORDER_KEY, "NaN".into());
        assert_eq!(
            load_zoom_extents_borders(Some(&storage)),
            ZoomExtentsBorders {
                parallel: default.parallel,
                perspective: 0.8,
            }
        );
        storage.set_string(PERSPECTIVE_BORDER_KEY, "0".into());
        assert_eq!(load_zoom_extents_borders(Some(&storage)), default);
    }

    #[test]
    fn viewport_tabs_visibility_round_trips_and_ignores_invalid_storage() {
        let mut storage = MemoryStorage::default();
        assert!(load_viewport_tabs_visible(Some(&storage)));
        let mut app = super::super::tests::test_app();
        app.viewport_tabs_visible = false;
        eframe::App::save(&mut app, &mut storage);
        assert!(!load_viewport_tabs_visible(Some(&storage)));
        storage.set_string(VIEWPORT_TABS_KEY, "invalid".into());
        assert!(load_viewport_tabs_visible(Some(&storage)));
    }

    #[test]
    fn viewport_tab_alignment_round_trips_and_ignores_invalid_storage() {
        let mut storage = MemoryStorage::default();
        assert_eq!(
            load_viewport_tab_alignment(Some(&storage)),
            ViewportTabAlignment::Bottom
        );
        let mut app = super::super::tests::test_app();
        app.viewport_tab_alignment = ViewportTabAlignment::Left;
        eframe::App::save(&mut app, &mut storage);
        assert_eq!(
            load_viewport_tab_alignment(Some(&storage)),
            ViewportTabAlignment::Left
        );
        storage.set_string(VIEWPORT_TAB_ALIGNMENT_KEY, "Diagonal".into());
        assert_eq!(
            load_viewport_tab_alignment(Some(&storage)),
            ViewportTabAlignment::Bottom
        );
    }
}
