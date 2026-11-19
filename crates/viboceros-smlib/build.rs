fn main() {
    use std::{env, path::PathBuf};
    assert_eq!(
        env::var("CARGO_CFG_TARGET_OS").unwrap(),
        "linux",
        "SMLib bridge currently supports Linux"
    );
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let upstream = root.join("../../third_party/usd-brep").canonicalize()
        .expect("Initialize usd-brep: GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep");
    assert!(
        upstream.join("source/SMLib/inc/SmBrep.h").is_file(),
        "USD BRep source is missing"
    );
    for input in [
        "native",
        "../viboceros-opennurbs/native/curve_discontinuity_query.h",
        "../../tools/usd_brep",
        "../../third_party/patches",
    ] {
        println!("cargo:rerun-if-changed={input}");
    }
    for input in ["VERSION", "source/SMLib", "source/SM_API"] {
        println!("cargo:rerun-if-changed={}", upstream.join(input).display());
    }
    println!("cargo:rerun-if-env-changed=VIBOCEROS_SMLIB_ASAN");
    let sanitize = match env::var("VIBOCEROS_SMLIB_ASAN").as_deref() {
        Ok("1") => true,
        Ok("0") | Err(_) => false,
        _ => panic!("VIBOCEROS_SMLIB_ASAN must be 0 or 1"),
    };
    let destination = cmake::Config::new(root.join("native"))
        .define("USD_BREP_ROOT", upstream)
        .define("VIBOCEROS_SMLIB_ASAN", if sanitize { "ON" } else { "OFF" })
        .profile("Release")
        .build();
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_CURVE_CONTINUITY_PROBE={}",
        destination
            .join("build/viboceros_smlib_curve_continuity_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_CIRCULAR_FILLET_RAILS_PROBE={}",
        destination
            .join("build/viboceros_smlib_circular_fillet_rails_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_TENSOR_BEZIER_PROBE={}",
        destination
            .join("build/viboceros_smlib_tensor_bezier_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_EXACT_SPLIT_PROBE={}",
        destination
            .join("build/viboceros_smlib_exact_split_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_POINT_INTERPOLATION_PROBE={}",
        destination
            .join("build/viboceros_smlib_point_interpolation_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_BASIS_EVALUATION_PROBE={}",
        destination
            .join("build/viboceros_smlib_basis_evaluation_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_CURVE_SEGMENT_PROBE={}",
        destination
            .join("build/viboceros_smlib_curve_segment_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_SURFACE_PATCH_PROBE={}",
        destination
            .join("build/viboceros_smlib_surface_patch_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_ISOCURVE_PAIR_PROBE={}",
        destination
            .join("build/viboceros_smlib_isocurve_pair_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_SURFACE_BLEND_PROBE={}",
        destination
            .join("build/viboceros_smlib_surface_blend_probe")
            .display()
    );
    println!(
        "cargo:rustc-env=VIBOCEROS_NATIVE_SURFACE_BLEND_CURVED_PROBE={}",
        destination
            .join("build/viboceros_smlib_surface_blend_curved_probe")
            .display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        destination.join("lib").display()
    );
    for library in ["viboceros_smlib", "SM_API", "SMLib"] {
        println!("cargo:rustc-link-lib=static={library}");
    }
    if sanitize {
        println!("cargo:rustc-link-lib=dylib=asan");
    }
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=tbb");
    println!("cargo:rustc-link-lib=dylib=tbbmalloc");
}
