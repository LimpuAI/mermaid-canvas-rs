# mermaid-canvas-rs

Backend-agnostic Rust Mermaid diagram renderer that outputs Canvas 2D instruction sequences. Designed for WASI Component Model.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- **Backend-agnostic** — Outputs Canvas 2D instruction sequences, decoupled from any rendering engine
- **Mermaid syntax** — Parses standard Mermaid diagram syntax into structured AST
- **Shape-based theming** — 6 semantic color slots mapped from node shapes, 5 built-in themes
- **Sugiyama layout** — Automatic hierarchical layout for flowchart-family diagrams
- **Layer-based rendering** — 7 layers (Background, Subgraphs, Edges, Nodes, Labels, Title, Annotation)
- **WASM-first** — Native WASI Component Model support (wasm32-wasip2 target)

## Diagram Types

| Diagram | Status | Description |
|---------|--------|-------------|
| Flowchart | ✅ Complete | Nodes, edges, subgraphs, multiple shapes |
| Class | ✅ Complete | Classes, relationships, visibility |
| State | ✅ Complete | States, transitions, composite states |
| ER | ✅ Complete | Entities, relationships, cardinality |
| Requirement | ✅ Complete | Requirements, constraints, relationships |
| Packet | ✅ Complete | Packet field diagrams |
| Sequence | ✅ Complete | Participants, messages, activations, notes, control blocks |

## Quick Start

**Native path (one-shot, backward compatible):**

```rust
use mermaid_canvas_wit;

let result = mermaid_canvas_wit::render(
    "flowchart TD\n    A[Start] -->|go| B{Choice?}\n    B -->|yes| C[(DB)]",
    Some("forest"),  // theme: default / dark / forest / nordic / cappuccino
)?;

// result.layers — layered drawing instructions
// result.width, result.height — canvas dimensions
// result.layers contains WitDrawCmd — render with any Canvas 2D backend
```

**Native path (v3 stateful session):**

```rust
use mermaid_canvas_wit::session::DiagramSession;
use mermaid_canvas_wit::wit_types::WitDiagramOptions;

let mut session = DiagramSession::new(
    "flowchart TD\n    A --> B".to_string(),
    Some(WitDiagramOptions { theme: Some("dark".into()), ..Default::default() }),
);
let steady = session.render(1.0)?;      // t=1 exact steady state
let enter = session.render(0.3)?;       // Tier 1 enter phase (fade+grow stagger)
let regions = session.hit_regions();    // AABB + node-id (host-side hit test)
session.update_source("flowchart LR\n    X --> Y".to_string())?;  // re-parse + replay enter
session.resize(400.0, 0.0);             // fit-to-width (shrink only)
```

**WASM Component (v3 resource session):**

```bash
# Build WASI Component (~1.8MB release)
cargo build -p mermaid-canvas-wit-wasm --target wasm32-wasip2 --release

# Run demo with WASM path
cargo run --bin demo-flowchart -- \
  --wasm target/wasm32-wasip2/release/mermaid_canvas_wit_wasm.wasm
```

## Themes

Built-in themes with shape-based semantic coloring — same node type gets the same color:

| Theme | Style | Primary Color |
|-------|-------|---------------|
| Default | Classic light | Blue `#dae8fc` |
| Dark | Dark cool tones | Dark blue-gray `#313244` |
| Forest | Deep green | Green `#2d5a27` |
| Nordic | Cool gray-blue minimal | Blue-gray `#dfe6ed` |
| Cappuccino | Warm brown tones | Latte `#e8d5c4` |

```bash
cargo run --bin demo-flowchart -- --theme cappuccino
cargo run --bin demo-themes -- --output ./out  # all 5 themes
```

## Crate Structure

| Crate | Description |
|-------|-------------|
| [mermaid-canvas-core](crates/mermaid-canvas-core) | Data types, drawing instructions, diagram parsers, interaction |
| [mermaid-canvas-component](crates/mermaid-canvas-component) | Sugiyama layout engine, diagram renderers, theme system |
| [mermaid-canvas-wit](crates/mermaid-canvas-wit) | WIT type conversion, lib_mode API |
| [mermaid-canvas-wit-wasm](crates/mermaid-canvas-wit-wasm) | WASI Component Model export (wit-bindgen 0.57) |
| [mermaid-canvas-demo](crates/mermaid-canvas-demo) | Desktop demo (tiny-skia + wasmtime host + winit window) |

## Demo

Seven demo binaries, each with native and WASM rendering paths:

```bash
# Native rendering
cargo run --bin demo-flowchart
cargo run --bin demo-class
cargo run --bin demo-state
cargo run --bin demo-er

# With theme and output
cargo run --bin demo-flowchart -- --theme forest --output out.png

# WASM rendering
cargo run --bin demo-flowchart -- \
  --wasm target/wasm32-wasip2/release/mermaid_canvas_wit_wasm.wasm \
  --theme dark --output wasm.png

# All themes at once
cargo run --bin demo-themes -- --output ./themes
```

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Host Application                                   │
│    ┌──────────────┐         ┌──────────────────┐    │
│    │ Direct Path  │         │   WASM Path      │    │
│    │DiagramSession│         │ wasmtime Host    │    │
│    └──────┬───────┘         └───────┬──────────┘    │
│           │                        │ mermaid:viz@3.0.0
│           ▼                        ▼ resource session
│    DrawCmd (native)    WitDrawCmd (WASM boundary)    │
│           │                          │               │
│           └──────────┬───────────────┘               │
│                      ▼                               │
│              TinySkiaRenderer                        │
│              (Canvas 2D → pixels)                    │
└─────────────────────────────────────────────────────┘
```

## WIT Protocol (v3 — resource session)

The component exports `mermaid:viz@3.0.0/diagram-renderer` with a stateful
`diagram` resource (constructor + six methods, session shape unchanged from
v2), using the shared `echodawn:canvas@2.0.0/draw` vocabulary for lossless
draw commands (Tier2 anim-desc multi-track / dash / line-cap / per-corner
radii / font-desc with font features / paint incl. linear gradients /
command ids / hover-effect):

| Method | Semantics |
|--------|-----------|
| `constructor(source, opts)` | parse + layout; parse errors surface at `render` |
| `update-source(source)` | re-parse + re-layout + replay enter phase |
| `resize(width, height)` | fit-to-width constraint (shrink only; diagram size is content-adaptive) |
| `set-state(state)` | hover brighten / selected outline (immediate) |
| `set-theme(theme)` | apply theme record (6 semantic color slots via `shape_slot`, plus `hover-color` and shape-axis `style-preset`: classic/signal-flow/blueprint/editorial — colors always come from host token injection slots) |
| `render(t)` | Tier 1 semantic phase: `t=1` exact steady state; `t∈[0,1)` enter stagger (nodes fade+grow, edges/labels fade); `disable` renders steady at any `t` |
| `hit-regions()` | node AABBs with node-id payload + optional declarative `hover-effect` (host-rendered; zero wasm calls per hit test) |

## Build & Test

```bash
# Build
cargo build --workspace

# Test (263 tests)
cargo test --workspace

# Build WASM component
cargo build -p mermaid-canvas-wit-wasm --target wasm32-wasip2 --release

# Lint
cargo clippy --workspace
```

## Packaging (standard plugin package)

宿主应用装载的标准插件包由 [aixpack](https://github.com/LimpuAI/aixpack)（LimpuAI 生态工具链，`cargo install --git https://github.com/LimpuAI/aixpack`）从本仓 `pack.toml` 产出：

```bash
aixpack --root <本仓> build       # wasip2 release 组件 → dist/staging/
aixpack --root <本仓> pkg --zip   # dist/<author>--<name>/ + 标准包 zip（唯一顶层）
aixpack --root <本仓> verify --wit-root crates/mermaid-canvas-wit/wit   --contracts-root <契约仓> --registry-root <登记处仓>
```

verify 三查：WIT 双侧一致（本仓 `crates/mermaid-canvas-wit/wit/mermaid-viz.wit` ↔ 契约权威副本）/ world ⊆ 扩展点登记处 / 包内哈希三方一致。主题资产包试点见 `packages/theme-sample/`（纯资产包，同为 aixpack 布局）。

## License

Licensed under the [MIT License](LICENSE).

Copyright (c) 2026 StarEcho Pte. Ltd.

快捷脚本：`scripts/verify-package.sh`（提交前三查门禁）、`scripts/install-hooks.sh`（pre-commit 安装器）。

工具链安装：`cargo install --git https://github.com/LimpuAI/aixpack`（安装后 `scripts/verify-package.sh` 自动优先 PATH；未安装时兜底兄弟 checkout 构建产物）。
