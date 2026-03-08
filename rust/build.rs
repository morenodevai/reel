use std::env;
use std::path::PathBuf;

/// Find and link the cpp-httplib static library built by llama-cpp-sys-2.
fn link_httplib() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    // OUT_DIR = .../target/{profile}/build/reel_core-{hash}/out
    // We need: .../target/{profile}/build/
    let build_dir = match out_path.parent().and_then(|p| p.parent()) {
        Some(d) => d,
        None => return,
    };

    let entries = match std::fs::read_dir(build_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("llama-cpp-sys-2-") {
            continue;
        }

        let httplib_dir = entry
            .path()
            .join("out")
            .join("build")
            .join("vendor")
            .join("cpp-httplib");

        // Check for both Unix (.a) and MSVC (.lib) static library names.
        let has_lib = httplib_dir.join("libcpp-httplib.a").exists()
            || httplib_dir.join("cpp-httplib.lib").exists();

        if has_lib {
            println!("cargo:rustc-link-search=native={}", httplib_dir.display());
            println!("cargo:rustc-link-lib=static=cpp-httplib");
            return;
        }
    }

    eprintln!("cargo:warning=Could not find cpp-httplib library in build directory");
}

/// Link platform-specific system libraries.
fn link_platform_libs() {
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

    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=ws2_32");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    link_httplib();
    link_platform_libs();
}
