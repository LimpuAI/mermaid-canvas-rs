//! 全主题对比演示
//!
//! 同一个 Flowchart 用所有内置主题分别渲染。
//!
//! 用法: demo-themes [--output <dir>]
//!
//! - 无参数：只弹窗显示最后一个主题
//! - `--output <dir>`：将所有主题 PNG 保存到指定目录

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = mermaid_canvas_demo::cli::CliArgs::parse();

    let source = mermaid_canvas_demo::sample_data::flowchart_sample();
    let themes: Vec<(&str, &str)> = vec![
        ("default", "Default"),
        ("dark", "Dark"),
        ("forest", "Forest"),
        ("nordic", "Nordic"),
        ("cappuccino", "Cappuccino"),
    ];

    let out_dir = cli.output.as_ref().map(|p| {
        let path = std::path::Path::new(p);
        if !path.exists() {
            std::fs::create_dir_all(path).ok();
        }
        path.to_path_buf()
    });

    let last_idx = themes.len() - 1;

    for (i, (theme_id, theme_label)) in themes.iter().enumerate() {
        let result = mermaid_canvas_wit::render(source, Some(theme_id))?;

        let w = result.width as u32;
        let h = result.height as u32;

        let mut renderer = mermaid_canvas_demo::renderer::TinySkiaRenderer::new(w, h)?;
        renderer.render_wit_layers(&result.layers);

        if let Some(ref dir) = out_dir {
            let png_path = dir.join(format!("demo_flowchart_{}.png", theme_id));
            renderer.save_png(&png_path)?;
            println!("Saved {} ({}x{}) [{}]", png_path.display(), w, h, theme_label);
        }

        if i == last_idx {
            let title = format!("Mermaid Canvas - Flowchart [{}]", theme_label);
            let app = mermaid_canvas_demo::app::DemoApp::new(&title, w, h);
            app.run(renderer.pixmap().clone())?;
        }
    }

    if out_dir.is_some() {
        println!("\nGenerated {} themed variants.", themes.len());
    }

    Ok(())
}
