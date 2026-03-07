use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = env::var("OUT_DIR").unwrap();

    // Navigate from our OUT_DIR up to the shared build/ directory.
    // OUT_DIR = .../target/{profile}/build/reel_core-{hash}/out
    // We need: .../target/{profile}/build/
    let out_path = PathBuf::from(&out_dir);
    let build_dir = out_path
        .parent()    // reel_core-HASH
        .and_then(|p| p.parent());  // build/

    if let Some(build_dir) = build_dir {
        // Find libcpp-httplib.a in any llama-cpp-sys-2 build directory
        let mut found = false;
        if let Ok(entries) = std::fs::read_dir(build_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("llama-cpp-sys-2-") {
                    continue;
                }

                let httplib_lib = entry.path()
                    .join("out")
                    .join("build")
                    .join("vendor")
                    .join("cpp-httplib")
                    .join("libcpp-httplib.a");

                if !httplib_lib.exists() {
                    continue;
                }

                eprintln!("cargo:warning=Found libcpp-httplib.a at {}", httplib_lib.display());

                // Extract objects from the archive
                let extract_dir = PathBuf::from(&out_dir).join("httplib_objects");
                let _ = std::fs::create_dir_all(&extract_dir);

                let status = Command::new("ar")
                    .args(["x", httplib_lib.to_str().unwrap()])
                    .current_dir(&extract_dir)
                    .status();

                if let Ok(s) = status {
                    if s.success() {
                        // Use cc to re-compile the extracted objects into a library
                        // that cargo will merge into our staticlib
                        let mut build = cc::Build::new();
                        build.cpp(true);
                        build.flag("-std=c++17");

                        let mut has_objects = false;
                        if let Ok(objs) = std::fs::read_dir(&extract_dir) {
                            for obj in objs.flatten() {
                                let p = obj.path();
                                if p.extension().map_or(false, |e| e == "o") {
                                    eprintln!("cargo:warning=Merging object: {}", p.display());
                                    build.object(&p);
                                    has_objects = true;
                                }
                            }
                        }

                        if has_objects {
                            // Write minimal source to satisfy cc::Build
                            let dummy = PathBuf::from(&out_dir).join("dummy_httplib_merge.cpp");
                            std::fs::write(&dummy, "// merged httplib objects\n").unwrap();
                            build.file(&dummy);
                            build.compile("httplib_merged");
                            found = true;
                        }
                    }
                }

                if found { break; }
            }
        }

        if !found {
            eprintln!("cargo:warning=Could not find libcpp-httplib.a in build directory");
        }
    }

    // macOS frameworks
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=MetalPerformanceShaders");
        println!("cargo:rustc-link-lib=framework=Accelerate");
        println!("cargo:rustc-link-lib=c++");
    }
}
