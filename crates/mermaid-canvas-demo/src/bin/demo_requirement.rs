//! Requirement Diagram 演示
//!
//! 用法: demo-requirement [--theme default|dark|forest] [--output <path>]
//!
//! 默认只弹窗显示；指定 `--output` 时保存 PNG。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = mermaid_canvas_demo::cli::CliArgs::parse();

    let source = mermaid_canvas_demo::sample_data::requirement_sample();

    let result = if let Some(ref wasm_path) = cli.wasm {
        eprintln!("Loading WASM component: {}", wasm_path);
        let mut host = mermaid_canvas_demo::wasm_host::WasmHost::from_file(wasm_path)?;
        host.render(source, cli.theme_arg())?
    } else {
        mermaid_canvas_wit::render(source, cli.theme_arg())?
    };

    let w = result.width as u32;
    let h = result.height as u32;

    let mut renderer = mermaid_canvas_demo::renderer::TinySkiaRenderer::new(w, h)?;
    renderer.render_wit_layers(&result.layers);

    if let Some(ref path) = cli.output {
        renderer.save_png(std::path::Path::new(path))?;
        println!("Saved {} ({}x{}) [{}]", path, w, h, cli.theme_name());
    }

    let title = format!("Mermaid Canvas - Requirement Diagram [{}]", cli.theme_name());
    let app = mermaid_canvas_demo::app::DemoApp::new(&title, w, h);
    app.run(renderer.pixmap().clone())
}
