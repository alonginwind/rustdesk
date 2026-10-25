fn build_c_impl() {
    let mut build = cc::Build::new();

    build.file("src/windows/wf_cliprdr.c");

    {
        build.flag_if_supported("-Wno-c++0x-extensions");
        build.flag_if_supported("-Wno-return-type-c-linkage");
        build.flag_if_supported("-Wno-invalid-offsetof");
        build.flag_if_supported("-Wno-unused-parameter");

        if build.get_compiler().is_like_msvc() {
            build.define("WIN32", "");
            // build.define("_AMD64_", "");
            build.flag("-Z7");
            build.flag("-GR-");
            // build.flag("-std:c++11");
        } else {
            build.flag("-fPIC");
            build.define("_WIN32_WINNT", "0x0601");
            // build.flag("-std=c++11");
            // build.flag("-include");
            // build.flag(&confdefs_path.to_string_lossy());
        }

        build.compile("mycliprdr");
    }

    println!("cargo:rustc-link-lib=uuid");
    println!("cargo:rerun-if-changed=src/windows/wf_cliprdr.c");
}

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "windows" {
        build_c_impl();
    }
}
