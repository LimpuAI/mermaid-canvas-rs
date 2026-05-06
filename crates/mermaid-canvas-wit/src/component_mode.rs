//! 独立组件模式

use super::wit_types::*;
use super::lib_mode;
use serde_json;

/// 组件输入配置
#[derive(Debug, serde::Deserialize)]
pub struct ComponentInput {
    /// Mermaid 源码
    pub source: String,
    /// 主题名称
    pub theme: Option<String>,
}

/// 组件输出结果
#[derive(Debug, serde::Serialize)]
pub struct ComponentOutput {
    /// 渲染结果
    pub result: WitRenderResult,
}

/// 运行组件（从 stdin 读取配置，输出到 stdout）
pub fn run_component() -> Result<(), String> {
    use std::io::{self, Read};

    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str)
        .map_err(|e| format!("Failed to read stdin: {}", e))?;

    let input: ComponentInput = serde_json::from_str(&input_str)
        .map_err(|e| format!("Failed to parse input JSON: {}", e))?;

    let result = lib_mode::render(&input.source, input.theme.as_deref())?;

    let output = ComponentOutput { result };
    let output_json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("Failed to serialize output: {}", e))?;

    println!("{}", output_json);
    Ok(())
}
