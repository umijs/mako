use std::{env, process::Command, str};

extern crate napi_build;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-env-changed=CI");

    match Command::new("git").args(["rev-parse", "HEAD"]).output() {
        Ok(out) if out.status.success() => println!(
            "cargo:warning=git HEAD: {}",
            str::from_utf8(&out.stdout).unwrap()
        ),
        _ => println!("cargo:warning=`git rev-parse HEAD` failed"),
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    napi_build::setup();

    // This is a workaround for napi always including a GCC specific flag.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        println!("cargo:rerun-if-env-changed=DEBUG_GENERATED_CODE");
        println!("cargo:rerun-if-env-changed=TYPE_DEF_TMP_PATH");
        println!("cargo:rerun-if-env-changed=CARGO_CFG_NAPI_RS_CLI_VERSION");

        println!("cargo:rustc-cdylib-link-arg=-undefined");
        println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
    }

    // Resolve a potential linker issue for unit tests on linux
    // https://github.com/napi-rs/napi-rs/issues/1782
    #[cfg(all(target_os = "linux", not(target_arch = "wasm32")))]
    println!("cargo:rustc-link-arg=-Wl,--warn-unresolved-symbols");

    #[cfg(not(target_arch = "wasm32"))]
    turbo_tasks_build::generate_register();

    Ok(())
}
