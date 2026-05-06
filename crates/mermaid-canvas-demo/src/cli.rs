//! 轻量 CLI 参数解析
//!
//! 用法: `<bin> [--theme <name>] [--output <path>] [--wasm <wasm-file>]`
//!
//! - `--theme`  可选，主题名称 (default / dark / forest / nordic / cappuccino)
//! - `--output` 可选，PNG 输出路径。未指定则只弹窗显示，不保存文件
//! - `--wasm`   可选，使用指定 .wasm 组件作为渲染后端（替代原生 Rust）

/// 解析后的 CLI 参数
pub struct CliArgs {
    pub theme: Option<String>,
    pub output: Option<String>,
    pub wasm: Option<String>,
}

impl CliArgs {
    /// 从 `std::env::args()` 解析
    pub fn parse() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        Self::parse_from(&args)
    }

    /// 从给定的参数切片解析
    pub fn parse_from(args: &[String]) -> Self {
        let mut theme: Option<String> = None;
        let mut output: Option<String> = None;
        let mut wasm: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--theme" => {
                    i += 1;
                    if i < args.len() {
                        theme = Some(args[i].clone());
                    }
                }
                "--output" => {
                    i += 1;
                    if i < args.len() {
                        output = Some(args[i].clone());
                    }
                }
                "--wasm" => {
                    i += 1;
                    if i < args.len() {
                        wasm = Some(args[i].clone());
                    }
                }
                _ => {} // ignore unknown
            }
            i += 1;
        }

        CliArgs { theme, output, wasm }
    }

    /// 主题名称，未指定则为 "default"
    pub fn theme_name(&self) -> &str {
        self.theme.as_deref().unwrap_or("default")
    }

    /// 主题参数，供 `mermaid_canvas_wit::render` 使用
    pub fn theme_arg(&self) -> Option<&str> {
        self.theme.as_deref()
    }

    /// 是否使用 WASM 后端
    pub fn use_wasm(&self) -> bool {
        self.wasm.is_some()
    }
}
