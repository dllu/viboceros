//! Active annotation style and the numeric settings used for plain-text labels.
use std::ffi::{CStr, c_char};
use std::sync::{Arc, OnceLock};

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
struct LengthFormat {
    meters_per_unit: f64,
    length_factor: f64,
    round_off: f64,
    resolution: i32,
    display: u32,
    zero_suppress: u32,
    decimal_separator: u32,
    leader: LeaderAppearance,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LeaderAppearance {
    pub text_height: f64,
    pub arrow_size: f64,
    pub landing_length: f64,
    pub text_gap: f64,
}

unsafe extern "C" {
    fn vibo_annotation_length_format(
        style: *const u8,
        count: usize,
        format: *mut LengthFormat,
    ) -> i32;
    fn vibo_annotation_format_coordinates(
        format: *const LengthFormat,
        model_meters_per_unit: f64,
        coordinates: *const f64,
        text: *mut c_char,
        capacity: usize,
    ) -> i32;
}

impl LengthFormat {
    fn from_archive(archive: &[u8]) -> Option<Self> {
        let mut format = Self::default();
        // SAFETY: the input slice is borrowed during the call and the output is initialized.
        let success =
            unsafe { vibo_annotation_length_format(archive.as_ptr(), archive.len(), &mut format) };
        (success == 1).then_some(format)
    }
}

/// An active OpenNURBS dimension style and its cached length-format settings.
/// An empty archive uses the OpenNURBS default style.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnotationStyle {
    archive: Arc<[u8]>,
    format: Option<LengthFormat>,
}

impl AnnotationStyle {
    pub fn from_archive(archive: &[u8]) -> Option<Self> {
        if archive.is_empty() {
            return Some(Self::default());
        }
        let format = LengthFormat::from_archive(archive)?;
        Some(Self {
            archive: Arc::from(archive),
            format: Some(format),
        })
    }

    pub fn archive(&self) -> &[u8] {
        &self.archive
    }

    fn format(&self) -> Option<&LengthFormat> {
        static DEFAULT: OnceLock<Option<LengthFormat>> = OnceLock::new();
        self.format.as_ref().or_else(|| {
            DEFAULT
                .get_or_init(|| LengthFormat::from_archive(&[]))
                .as_ref()
        })
    }

    pub fn leader_appearance(&self) -> LeaderAppearance {
        self.format()
            .map_or_else(LeaderAppearance::default, |format| format.leader)
    }

    pub fn format_coordinates(
        &self,
        coordinates: [f64; 3],
        model_meters_per_unit: Option<f64>,
    ) -> Option<String> {
        let format = self.format()?;
        let mut output = [0_u8; 4096];
        // SAFETY: all pointers cover the specified lengths for the synchronous call.
        let success = unsafe {
            vibo_annotation_format_coordinates(
                format,
                model_meters_per_unit.unwrap_or(0.0),
                coordinates.as_ptr(),
                output.as_mut_ptr().cast(),
                output.len(),
            )
        };
        if success != 1 {
            return None;
        }
        Some(
            CStr::from_bytes_until_nul(&output)
                .ok()?
                .to_str()
                .ok()?
                .to_owned(),
        )
    }
}
