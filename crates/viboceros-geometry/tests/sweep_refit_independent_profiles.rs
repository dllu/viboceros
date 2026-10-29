#[path = "support/section_refit.rs"]
#[allow(dead_code)]
mod section_refit;

// Independent expensive proofs run concurrently through the normal test runner.
macro_rules! cases {
    ($($name:ident = $index:literal;)*) => {
        const _: () = {
            let indices = [$($index,)*];
            assert!(indices.len() == 16);
            let mut i = 0;
            while i < indices.len() { assert!(indices[i] == i); i += 1; }
        };
        $(#[test] fn $name() {
            section_refit::require_complete_cases("sweep1_refit_independent_profiles",16,$index..$index+1);
        })*
    }
}
cases! {
    asymmetric_quadratic_fine_single = 0;
    asymmetric_quadratic_fine_pair = 1;
    asymmetric_quadratic_coarser_single = 2;
    asymmetric_quadratic_coarser_pair = 3;
    spatial_quadratic_fine_single = 4;
    spatial_quadratic_fine_pair = 5;
    spatial_quadratic_coarser_single = 6;
    spatial_quadratic_coarser_pair = 7;
    translated_circle_fine_single = 8;
    translated_circle_fine_pair = 9;
    translated_circle_coarser_single = 10;
    translated_circle_coarser_pair = 11;
    closed_cubic_fine_single = 12;
    closed_cubic_fine_pair = 13;
    closed_cubic_coarser_single = 14;
    closed_cubic_coarser_pair = 15;
}
