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
    style_height: f64,
    text_alignment: u32,
    text_in_view: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LeaderAppearance {
    pub text_height: f64,
    pub arrow_size: f64,
    pub landing_length: f64,
    pub text_gap: f64,
    text_rotation: f64,
    curve_type: u32,
    arrow_type: u32,
    angle_style: u32,
    draw_forward: u32,
    arrow_block: [u8; 16],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderCurveType {
    None,
    Polyline,
    Spline,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LeaderTextAngle {
    Horizontal,
    Aligned,
    Rotated(f64),
}

impl LeaderAppearance {
    pub fn curve_type(self) -> LeaderCurveType {
        match self.curve_type {
            0 => LeaderCurveType::None,
            2 => LeaderCurveType::Spline,
            _ => LeaderCurveType::Polyline,
        }
    }

    pub fn text_angle(self) -> LeaderTextAngle {
        match self.angle_style {
            1 => LeaderTextAngle::Aligned,
            2 => LeaderTextAngle::Rotated(self.text_rotation),
            _ => LeaderTextAngle::Horizontal,
        }
    }

    pub fn draws_forward(self) -> bool {
        self.draw_forward != 0
    }

    pub fn arrow_block_id(self) -> Option<[u8; 16]> {
        (self.arrow_block != [0; 16]).then_some(self.arrow_block)
    }

    pub fn arrow_outline(self) -> &'static [[f64; 2]] {
        let mut points = std::ptr::null();
        // SAFETY: OpenNURBS returns immutable static pairs for its built-in arrows.
        let count = unsafe { vibo_annotation_arrowhead(self.arrow_type, &mut points) };
        if points.is_null() || !(3..=24).contains(&count) {
            return &[];
        }
        // SAFETY: the returned static array contains exactly count pairs of doubles.
        unsafe { std::slice::from_raw_parts(points.cast(), count as usize) }
    }
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
    fn vibo_annotation_arrowhead(arrow_type: u32, points: *mut *const f64) -> u32;
    fn vibo_annotation_set_arrow_block(
        style: *const u8,
        count: usize,
        id: *const u8,
        output: *mut u8,
        capacity: usize,
    ) -> usize;
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

    pub fn text_alignment(&self) -> TextAlignment {
        match self.format().map(|format| format.text_alignment) {
            Some(1) => TextAlignment::Center,
            Some(2) => TextAlignment::Right,
            _ => TextAlignment::Left,
        }
    }

    pub fn text_in_view(&self) -> bool {
        self.format().is_some_and(|format| format.text_in_view != 0)
    }

    pub fn text_style_height(&self) -> f64 {
        self.format().map_or(1.0, |format| format.style_height)
    }

    /// Change the referenced definition while retaining the other style settings.
    pub fn with_leader_arrow_block(&self, id: [u8; 16]) -> Option<Self> {
        let previous = self.leader_appearance().arrow_block_id()?;
        if previous == id {
            return Some(self.clone());
        }
        let write = |buffer: &mut [u8]| {
            // SAFETY: the archive, 16 UUID bytes and output span live across the call.
            unsafe {
                vibo_annotation_set_arrow_block(
                    self.archive.as_ptr(),
                    self.archive.len(),
                    id.as_ptr(),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                )
            }
        };
        let mut buffer = vec![0; self.archive.len()];
        let count = write(&mut buffer);
        if count > buffer.len() {
            buffer.resize(count, 0);
            if write(&mut buffer) != count {
                return None;
            }
        }
        (count != 0)
            .then(|| Self::from_archive(&buffer[..count]))
            .flatten()
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
