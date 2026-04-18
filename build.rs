use std::path::PathBuf;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

fn main() {
    // Compile Slint sources. main_window.slint is the compile-entry hub
    // that re-exports every Window, global, and struct used from Rust
    // (OverlayWindow, WorkspaceWindow, PaletteWindow, PencilWindow,
    // AppBridge, ModeInfo, ChainInfo, ...). slint-build emits Rust code
    // which slint::include_modules!() picks up at build time.
    //
    // Use the "native" style — we paint every widget ourselves (warm
    // editorial palette driven by theme.slint), so the prior
    // "fluent-dark" style was never rendered but still bloated generated
    // code with its full widget implementations.
    let slint_config = slint_build::CompilerConfiguration::new().with_style("native".into());
    slint_build::compile_with_config("src/ui/slint/main_window.slint", slint_config)
        .expect("slint compilation failed");
    println!("cargo:rerun-if-changed=src/ui/slint");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let svg_path = manifest_dir.join("resources/icons/plume.svg");
    // The tray PNG is the only rasterised artifact we actually consume —
    // `src/platform/tray.rs` picks it up via
    // `include_bytes!(concat!(env!("OUT_DIR"), "/quill-tray-32.png"))`.
    // A multi-size `quill.ico` was generated here previously but nothing
    // linked it into the exe (no `winres`/`embed_resource` dep), so the
    // work was wasted and the README's "embedded icon" claim was wrong.
    let out_dir =
        PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts"));

    // Re-run only when the source SVG or this script changes
    println!("cargo:rerun-if-changed={}", svg_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    // theme.slint imports TTFs from resources/fonts/ and workspace.slint
    // references SVGs under resources/textures/. Without these, a font or
    // texture change wouldn't trigger a rebuild and the embedded data would
    // stay stale until `cargo clean`.
    println!("cargo:rerun-if-changed=resources/fonts");
    println!("cargo:rerun-if-changed=resources/textures");

    let svg_bytes = std::fs::read(&svg_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", svg_path.display()));

    let tree =
        Tree::from_data(&svg_bytes, &Options::default()).expect("plume.svg is not valid SVG");

    let svg_size = tree.size();

    // Render ONE 32×32 PNG for the tray. No ICO, no other sizes.
    let size: u32 = 32;
    let mut pixmap = Pixmap::new(size, size).expect("allocate pixmap");
    let scale_x = size as f32 / svg_size.width();
    let scale_y = size as f32 / svg_size.height();
    let transform = Transform::from_scale(scale_x, scale_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let png_path = out_dir.join("quill-tray-32.png");
    let png_bytes = pixmap.encode_png().expect("encode quill-tray-32.png");
    std::fs::write(&png_path, png_bytes).expect("write quill-tray-32.png");
}
