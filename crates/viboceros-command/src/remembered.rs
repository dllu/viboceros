//! Command-instance preferences, deliberately outside model history.
//!
//! A registry owns its command instances, so aliases and documents executed by
//! that registry share preferences, while independent registries remain isolated.
//! No lock is held while parsing, evaluating geometry, or mutating a document.
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct Remembered<T>(Mutex<T>);

impl<T: Copy> Remembered<T> {
    pub(super) fn get(&self) -> T {
        *self
            .0
            .lock()
            .expect("preference locks never invoke user code")
    }

    pub(super) fn set(&self, value: T) {
        *self
            .0
            .lock()
            .expect("preference locks never invoke user code") = value;
    }
}
