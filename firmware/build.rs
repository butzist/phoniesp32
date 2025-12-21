use std::fs;
use std::path::Path;
use walkdir::WalkDir;

fn main() {
    linker_be_nice();
    println!("cargo:rustc-link-arg=-Tdefmt.x");
    // make sure linkall.x is the last linker script (otherwise might cause problems with flip-link)
    println!("cargo:rustc-link-arg=-Tlinkall.x");

    generate_assets();
}

fn generate_assets() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut code = String::new();
    code.push_str("use picoserve::{response::File, routing};\n\n");

    // Generate add_asset_routes function
    code.push_str("pub fn add_asset_routes<PR, State>(router: picoserve::Router<PR, State>) -> picoserve::Router<impl picoserve::routing::PathRouter<State>, State>\n");
    code.push_str("where\n");
    code.push_str("    PR: picoserve::routing::PathRouter<State>,\n");
    code.push_str("{\n");
    code.push_str("    router\n");

    for entry in WalkDir::new("public/assets")
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension()
                && ext == "gz"
            {
                let rel_path = path.strip_prefix("public").unwrap();
                let rel_str = rel_path.to_str().unwrap();
                let route_path = format!("/{}", rel_str.trim_end_matches(".gz"));
                let file_stem = Path::new(rel_str).file_stem().unwrap().to_str().unwrap();
                let mime = mime_guess::from_path(file_stem)
                    .first_or_octet_stream()
                    .to_string();
                code.push_str(&format!("        .route(\"{}\", routing::get_service(File::with_content_type_and_headers(\"{}\", include_bytes!(\"{}/public/{}\"), &[(\"Content-Encoding\", \"gzip\")])))\n", route_path, mime, manifest_dir, rel_str));
            }
        }
    }

    code.push_str("}\n");

    fs::write(format!("{}/assets.rs", out_dir), code).unwrap();
    println!("cargo:rerun-if-changed=public");
}

fn linker_be_nice() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                "_defmt_timestamp" => {
                    eprintln!();
                    eprintln!(
                        "💡 `defmt` not found - make sure `defmt.x` is added as a linker script and you have included `use defmt_rtt as _;`"
                    );
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("💡 Is the linker script `linkall.x` missing?");
                    eprintln!();
                }
                "esp_rtos_initialized" | "esp_rtos_yield_task" | "esp_rtos_task_create" => {
                    eprintln!();
                    eprintln!(
                        "💡 `esp-radio` has no scheduler enabled. Make sure you have initialized `esp-rtos` or provided an external scheduler."
                    );
                    eprintln!();
                }
                "embedded_test_linker_file_not_added_to_rustflags" => {
                    eprintln!();
                    eprintln!(
                        "💡 `embedded-test` not found - make sure `embedded-test.x` is added as a linker script for tests"
                    );
                    eprintln!();
                }
                _ => (),
            },
            // we don't have anything helpful for "missing-lib" yet
            _ => {
                std::process::exit(1);
            }
        }

        std::process::exit(0);
    }

    println!(
        "cargo:rustc-link-arg=--error-handling-script={}",
        std::env::current_exe().unwrap().display()
    );
}
