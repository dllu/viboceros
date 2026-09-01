use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let open_nurbs = crate_dir.join("../../third_party/opennurbs");
    if !open_nurbs.join("opennurbs.h").is_file() {
        panic!(
            "OpenNURBS is missing; run `git submodule update --init --recursive` before building"
        );
    }

    println!("cargo:rerun-if-changed=native/CMakeLists.txt");
    println!("cargo:rerun-if-changed=native/viboceros_opennurbs.cpp");
    println!("cargo:rerun-if-changed=native/viboceros_opennurbs.h");
    println!("cargo:rerun-if-changed={}", open_nurbs.display());

    let destination = cmake::Config::new(crate_dir.join("native"))
        .define("OPENNURBS_ROOT", absolute(&open_nurbs))
        .define("BUILD_TESTING", "OFF")
        .profile("Release")
        .build_target("viboceros_opennurbs")
        .build();
    let build = destination.join("build");

    link_search(&build);
    link_search(build.join("opennurbs"));
    link_search(build.join("opennurbs/zlib"));
    println!("cargo:rustc-link-lib=static=viboceros_opennurbs");
    // OpenNURBS and its prefixed zlib have circular static references (the
    // allocation hooks live in OpenNURBS). Loading the archive as a whole is
    // portable across Cargo's supported linkers and still permits section GC.
    println!("cargo:rustc-link-lib=static:+whole-archive=opennurbsStatic");

    let target = env::var("CARGO_CFG_TARGET_OS").expect("target OS");
    match target.as_str() {
        "linux" | "android" => {
            link_search(build.join("opennurbs/freetype263"));
            link_search(build.join("opennurbs/android_uuid"));
            println!("cargo:rustc-link-lib=static:+whole-archive=zlib");
            println!("cargo:rustc-link-lib=static:+whole-archive=opennurbs_public_freetype");
            println!("cargo:rustc-link-lib=static:+whole-archive=android_uuid");
            println!("cargo:rustc-link-lib=dylib=stdc++");
            if target == "android" {
                println!("cargo:rustc-link-lib=dylib=android");
            }
        }
        "macos" | "ios" => {
            println!("cargo:rustc-link-lib=static:+whole-archive=zlib");
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=CoreGraphics");
            println!("cargo:rustc-link-lib=framework=CoreText");
            println!("cargo:rustc-link-lib=framework=Foundation");
        }
        "windows" => {
            println!("cargo:rustc-link-lib=static:+whole-archive=zlib");
            println!("cargo:rustc-link-lib=dylib=Shlwapi");
            println!("cargo:rustc-link-lib=dylib=Usp10");
            println!("cargo:rustc-link-lib=dylib=Rpcrt4");
        }
        other => panic!("the OpenNURBS bridge does not support target OS '{other}' yet"),
    }
}

fn absolute(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|error| panic!("failed to resolve {}: {error}", path.display()))
}

fn link_search(path: impl AsRef<Path>) {
    println!("cargo:rustc-link-search=native={}", path.as_ref().display());
}
