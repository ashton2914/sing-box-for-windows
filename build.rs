//! Build script:
//! 1. Rasterize `assets/icon.svg` into a 256x256 raw RGBA buffer used by
//!    eframe at runtime as the window/taskbar icon.
//! 2. Build a multi-size `.ico` (16, 32, 48, 64, 128, 256) and embed it as a
//!    Windows resource so the .exe file shows the icon in Explorer.
//!
//! The rasterization deps live under `[build-dependencies]` so they don't
//! bloat the final binary.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.svg");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    let svg_data = fs::read("assets/icon.svg").expect("missing assets/icon.svg");

    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg_data, &opt).expect("invalid SVG");
    let svg_size = tree.size();

    // ---- 256x256 RGBA for the egui window icon ----
    let window_pix = render_svg(&tree, svg_size, 256);
    fs::write(out_dir.join("icon.rgba"), window_pix.data()).expect("write icon.rgba");

    // ---- multi-size .ico for the Windows exe resource ----
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 32, 48, 64, 128, 256] {
        let p = render_svg(&tree, svg_size, size);
        let img = ico::IconImage::from_rgba_data(size, size, p.data().to_vec());
        let entry = ico::IconDirEntry::encode(&img).expect("encode ico entry");
        icon_dir.add_entry(entry);
    }
    let ico_path = out_dir.join("icon.ico");
    let ico_file = fs::File::create(&ico_path).expect("create icon.ico");
    icon_dir.write(ico_file).expect("write icon.ico");

    // ---- embed as Windows resource so the exe carries the icon ----
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico_path.to_str().expect("ico path utf8"));
        if let Err(e) = res.compile() {
            // Don't fail the whole build if winresource tooling is missing —
            // the runtime window icon will still work.
            println!(
                "cargo:warning=embedding Windows resource failed: {e}; the exe \
                 file in Explorer will lack the icon, but the running window will still show it."
            );
        }
    }
}

fn render_svg(tree: &usvg::Tree, svg_size: usvg::Size, target_px: u32) -> tiny_skia::Pixmap {
    let mut pix = tiny_skia::Pixmap::new(target_px, target_px).expect("alloc pixmap");
    let scale_x = target_px as f32 / svg_size.width();
    let scale_y = target_px as f32 / svg_size.height();
    let scale = scale_x.min(scale_y);
    let dx = (target_px as f32 - svg_size.width() * scale) / 2.0;
    let dy = (target_px as f32 - svg_size.height() * scale) / 2.0;
    let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);
    resvg::render(tree, transform, &mut pix.as_mut());
    pix
}
