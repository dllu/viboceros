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
    println!("cargo:rerun-if-changed=../../tools/usd_brep/isocurve_pair_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/surface_patch_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/curve_segment_limits_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/basis_evaluation_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/point_interpolation_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/exact_split_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/tensor_bezier_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/curve_contact_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/drop_curve_accuracy_probe.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/circular_fillet_rails_probe.cpp");
    println!("cargo:rerun-if-changed=native/face_creases.cpp");
    println!("cargo:rerun-if-changed=../../tools/usd_brep/planarity_zone_probe.cpp");
    println!("cargo:rerun-if-changed=../../third_party/patches/usd-brep-protected-topology.patch");
    println!(
        "cargo:rerun-if-changed=../../third_party/patches/usd-brep-closed-fillet-corner-state.patch"
    );
    println!(
        "cargo:rerun-if-changed=../../third_party/patches/usd-brep-g2-blend-rail-parameterization.patch"
    );
    println!(
        "cargo:rerun-if-changed=../../third_party/patches/usd-brep-surface-drop-start-normal.patch"
    );
    for patch in [
        "usd-brep-proto-edge-endpoint-links.patch",
        "usd-brep-planarity-zone.patch",
        "usd-brep-isolated-loop-owner.patch",
        "usd-brep-tapered-sliver-domains.patch",
        "usd-brep-curve-fillet-sides.patch",
        "usd-brep-curve-fillet-seed.patch",
        "usd-brep-curve-fillet-seed-header.patch",
        "usd-brep-curve-intersection-distance.patch",
        "usd-brep-curve-intersection-distance-solver.patch",
        "usd-brep-curve-intersection-distance-header.patch",
        "usd-brep-curve-intersection-distance.cmake",
        "usd-brep-curve-self-intersection-pairs.patch",
        "usd-brep-curve-self-intersection-pairs.cmake",
        "usd-brep-curve-contact-tolerance.patch",
        "usd-brep-curve-contact-tolerance.cmake",
        "usd-brep-drop-curve-accuracy.patch",
        "usd-brep-drop-curve-accuracy.cmake",
        "usd-brep-copy-curve-basis.patch",
        "usd-brep-copy-surface-basis.patch",
        "usd-brep-copy-basis.cmake",
        "usd-brep-surface-transpose-basis.patch",
        "usd-brep-face-transpose-uv.patch",
        "usd-brep-exact-uv-transpose.cmake",
        "usd-brep-isocurve-output-basis.patch",
        "usd-brep-isocurve-uv-line.patch",
        "usd-brep-isocurve-limits.patch",
        "usd-brep-isocurve-pairs.cmake",
        "usd-brep-surface-patch-region.patch",
        "usd-brep-surface-patch-output.patch",
        "usd-brep-surface-patch.cmake",
        "usd-brep-curve-segment-limits.patch",
        "usd-brep-curve-segment-limits.cmake",
        "usd-brep-basis-evaluation.patch",
        "usd-brep-curve-evaluation-basis.patch",
        "usd-brep-basis-evaluation.cmake",
        "usd-brep-point-interpolation.patch",
        "usd-brep-point-interpolation.cmake",
        "usd-brep-curve-exact-split.patch",
        "usd-brep-surface-exact-split.patch",
        "usd-brep-exact-split-span.h",
        "usd-brep-exact-split.cmake",
        "usd-brep-tensor-bezier.patch",
        "usd-brep-tensor-bezier.cmake",
        "usd-brep-surface-fillet-domain.patch",
        "usd-brep-surface-fillet-header.patch",
        "usd-brep-surface-fillet.cmake",
        "usd-brep-circular-fillet-rail-parameters.patch",
        "usd-brep-circular-fillet-rail-parameters.cmake",
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
