//! CAD file-format readers and writers.

mod stl;

pub use stl::{StlError, StlFormat, read_stl, read_stl_file, write_stl, write_stl_file};
