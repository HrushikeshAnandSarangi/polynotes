use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let whisper_dir = manifest_dir.join("whisper.cpp");

    if !whisper_dir.join("src").join("whisper.cpp").exists() {
        panic!(
            "whisper.cpp submodule not found at {:?}.\n\
            Run: git submodule update --init --recursive",
            whisper_dir
        );
    }

    // Detect if we are using MSVC (Windows) or GNU/Clang toolchain
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let is_msvc = target_env == "msvc";
    let is_x86_64 = target_arch == "x86_64";

    // ── C sources (ggml core + CPU backend C files + x86 arch quants) ────────
    let mut c_build = cc::Build::new();
    c_build
        .file(whisper_dir.join("ggml/src/ggml.c"))
        .file(whisper_dir.join("ggml/src/ggml-alloc.c"))
        .file(whisper_dir.join("ggml/src/ggml-quants.c"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/ggml-cpu.c"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/quants.c"))
        // x86-specific arch quant implementations (ggml_vec_dot_q*_K_q8_K etc.)
        .file(whisper_dir.join("ggml/src/ggml-cpu/arch/x86/quants.c"))
        .include(whisper_dir.join("ggml/include"))
        .include(whisper_dir.join("ggml/src"))
        .include(whisper_dir.join("ggml/src/ggml-cpu"))
        .include(whisper_dir.join("ggml/src/ggml-cpu/arch/x86"))
        .define("GGML_VERSION", "\"0.0.0\"")
        .define("GGML_COMMIT", "\"unknown\"")
        .define("GGML_USE_CPU", None); // enable CPU backend in ggml-cpu.c
    if !is_msvc {
        c_build
            .flag_if_supported("-O3")
            .flag_if_supported("-std=c11")
            .flag_if_supported("-pthread")
            .flag_if_supported("-fPIC")
            .define("_GNU_SOURCE", None);
    } else {
        c_build.flag("/O2");
    }
    c_build.compile("ggml_c");

    // ── C++ sources (ggml backend, dl loader, CPU backend, opt, threading) ──
    let mut cpp_build = cc::Build::new();
    cpp_build
        .cpp(true)
        // ggml core C++ wrapper
        .file(whisper_dir.join("ggml/src/ggml.cpp"))
        // backend infrastructure
        .file(whisper_dir.join("ggml/src/ggml-backend.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-backend-dl.cpp")) // dl_error / dl_load_library
        .file(whisper_dir.join("ggml/src/ggml-backend-reg.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-opt.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-threading.cpp"))
        // CPU backend (ggml_backend_cpu_reg + all compute kernels)
        .file(whisper_dir.join("ggml/src/ggml-cpu/ggml-cpu.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/ops.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/unary-ops.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/binary-ops.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/vec.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/traits.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/repack.cpp"))
        // x86 arch C++ files (CPU feature detection, SIMD repacking)
        .file(whisper_dir.join("ggml/src/ggml-cpu/arch/x86/cpu-feats.cpp"))
        .file(whisper_dir.join("ggml/src/ggml-cpu/arch/x86/repack.cpp"))
        // includes
        .include(whisper_dir.join("ggml/include"))
        .include(whisper_dir.join("ggml/src"))
        .include(whisper_dir.join("ggml/src/ggml-cpu"))
        .include(whisper_dir.join("ggml/src/ggml-cpu/arch/x86"))
        .define("GGML_VERSION", "\"0.0.0\"")
        .define("GGML_COMMIT", "\"unknown\"")
        .define("GGML_USE_CPU", None); // enable the CPU backend
    if is_msvc {
        cpp_build.flag("/std:c++17").flag("/EHsc").flag("/O2");
        if is_x86_64 {
            cpp_build.flag("/arch:AVX2");
        }
    } else {
        cpp_build
            .flag_if_supported("-O3")
            .flag_if_supported("-std=c++17")
            .flag_if_supported("-pthread")
            .flag_if_supported("-fPIC")
            .define("_GNU_SOURCE", None);
        if is_x86_64 {
            cpp_build
                .flag_if_supported("-mavx2")
                .flag_if_supported("-mfma");
        }
    }
    cpp_build.compile("ggml_cpp");

    // ── whisper.cpp main library ───────────────────────────────────────────
    let mut whisper_build = cc::Build::new();
    whisper_build
        .cpp(true)
        .file(whisper_dir.join("src/whisper.cpp"))
        .include(whisper_dir.join("include"))
        .include(whisper_dir.join("ggml/include"))
        .include(whisper_dir.join("ggml/src"))
        .define("WHISPER_VERSION", "\"0.0.0\"")
        .define("WHISPER_COMMIT", "\"unknown\"");
    if is_msvc {
        whisper_build.flag("/std:c++17").flag("/EHsc").flag("/O2");
        if is_x86_64 {
            whisper_build.flag("/arch:AVX2");
        }
    } else {
        whisper_build
            .flag_if_supported("-O3")
            .flag_if_supported("-std=c++17")
            .flag_if_supported("-pthread")
            .flag_if_supported("-fPIC");
        if is_x86_64 {
            whisper_build
                .flag_if_supported("-mavx2")
                .flag_if_supported("-mfma");
        }
    }
    whisper_build.compile("whisper");

    // ── bindgen ───────────────────────────────────────────────────────────
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", whisper_dir.join("include").display()))
        .clang_arg(format!("-I{}", whisper_dir.join("ggml/include").display()))
        .allowlist_function("whisper_.*")
        .allowlist_type("whisper_.*")
        .allowlist_var("WHISPER_.*")
        .blocklist_type("__.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Failed to generate FFI bindings");
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Failed to write bindings.rs");

    // ── link libraries ────────────────────────────────────────────────────
    if is_msvc {
        // ggml CPU backend calls Windows Registry APIs for CPU feature detection
        println!("cargo:rustc-link-lib=Advapi32");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
    }
}
