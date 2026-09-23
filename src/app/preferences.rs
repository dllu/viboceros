//! Application settings stored by eframe, separate from model undo history.

use viboceros_command::interface::ZoomScale;

use super::DEFAULT_ZOOM_SCALE;
use crate::viewport::ZoomExtentsBorders;

const ZOOM_SCALE_KEY: &str = "viboceros.view.zoom_scale.v1";
const PARALLEL_BORDER_KEY: &str = "viboceros.view.zoom_extents_parallel_border.v1";
const PERSPECTIVE_BORDER_KEY: &str = "viboceros.view.zoom_extents_perspective_border.v1";

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
}
