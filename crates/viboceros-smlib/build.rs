fn main() {
    println!(
        "cargo:rerun-if-changed=../../third_party/patches/usd-brep-knot-removal-bound-error.patch"
    );
    #[cfg(feature = "native")]
    build_native();
}

#[cfg(feature = "native")]
fn build_native() {
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
    println!("cargo:rerun-if-changed=native");
    for patch in [
        "usd-brep-curve-fillet-sides.patch",
        "usd-brep-curve-fillet-seed.patch",
        "usd-brep-curve-fillet-seed-header.patch",
        "usd-brep-curve-intersection-distance.patch",
        "usd-brep-curve-intersection-distance-solver.patch",
        "usd-brep-curve-intersection-distance-header.patch",
        "usd-brep-curve-intersection-distance.cmake",
        "usd-brep-curve-self-intersection-pairs.patch",
        "usd-brep-curve-self-intersection-pairs.cmake",
    ] {
        println!("cargo:rerun-if-changed=../../third_party/patches/{patch}");
    }
    println!("cargo:rerun-if-changed=../../third_party/patches/usd-brep-interior-point.patch");
    println!(
        "cargo:rerun-if-changed=../../third_party/patches/usd-brep-blend-rail-parameterization.patch"
    );
    println!("cargo:rerun-if-changed=../../tools/usd_brep/surface_blend_probe.cpp");
    println!(
        "cargo:rerun-if-changed=../../third_party/patches/usd-brep-blend-derivative-scaling.patch"
    );
    println!("cargo:rerun-if-changed=../../tools/usd_brep/surface_blend_curved_probe.cpp");
    println!(
        "cargo:rerun-if-changed=../../third_party/patches/usd-brep-extension-break-allocation.patch"
    );
    println!(
        "cargo:rerun-if-changed={}",
        upstream.join("source/SMLib").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        upstream.join("source/SM_API").display()
    );
    let destination = cmake::Config::new(root.join("native"))
        .define("USD_BREP_ROOT", upstream)
        .profile("Release")
        .build();
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
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=tbb");
    println!("cargo:rustc-link-lib=dylib=tbbmalloc");
}
