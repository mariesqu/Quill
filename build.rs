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

    // ── Tray PNG ──────────────────────────────────────────────────────────
    // Consumed at runtime by `platform/tray.rs` via
    // `include_bytes!(concat!(env!("OUT_DIR"), "/quill-tray-32.png"))`.
    let tray_size: u32 = 32;
    let mut tray_pix = Pixmap::new(tray_size, tray_size).expect("allocate tray pixmap");
    let tray_transform = Transform::from_scale(
        tray_size as f32 / svg_size.width(),
        tray_size as f32 / svg_size.height(),
    );
    resvg::render(&tree, tray_transform, &mut tray_pix.as_mut());
    let tray_png_path = out_dir.join("quill-tray-32.png");
    std::fs::write(
        &tray_png_path,
        tray_pix.encode_png().expect("encode quill-tray-32.png"),
    )
    .expect("write quill-tray-32.png");

    // ── Multi-size .ico for the Windows exe resource ──────────────────────
    // Rendered at every standard Windows icon size so Explorer, taskbar,
    // Alt-Tab picker, and the MSI installer all have a crisp glyph to
    // sample. The .ico is embedded into the exe via winresource below;
    // runtime Slint window taskbar icons are set separately by each
    // Window's `icon: @image-url(...)` declaration.
    let ico_sizes: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &sz in &ico_sizes {
        let mut pix = Pixmap::new(sz, sz).expect("allocate ico pixmap");
        let t = Transform::from_scale(sz as f32 / svg_size.width(), sz as f32 / svg_size.height());
        resvg::render(&tree, t, &mut pix.as_mut());
        let img = ico::IconImage::from_rgba_data(sz, sz, pix.data().to_vec());
        icon_dir.add_entry(ico::IconDirEntry::encode(&img).expect("encode ico entry"));
    }
    let ico_path = out_dir.join("quill.ico");
    let mut ico_file = std::fs::File::create(&ico_path).expect("create quill.ico");
    icon_dir.write(&mut ico_file).expect("write quill.ico");
    drop(ico_file);

    // ── Embed the .ico into the exe resource table ────────────────────────
    // Check the TARGET OS (not the build host) so cross-compilation from
    // Linux/macOS to Windows still embeds the resource. On a non-Windows
    // target this block is a no-op — the .ico above is still generated
    // because cargo-wix references it for the MSI installer branding.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico_path.to_str().expect("ico path must be UTF-8"));
        // Metadata shown in Explorer's Properties dialog and the MSI's
        // Add/Remove Programs entry.
        res.set("ProductName", "Quill");
        res.set("FileDescription", env!("CARGO_PKG_DESCRIPTION"));
        res.set("CompanyName", "Quill");
        res.set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            // Non-fatal — an exe without the embedded icon still runs.
            // Surface via cargo warning so the developer notices.
            println!("cargo:warning=winresource failed to embed icon: {e}");
        }
    }
}
