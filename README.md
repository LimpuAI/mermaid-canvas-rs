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
| Sequence | 🔧 Placeholder | Coming soon |

## Quick Start

**Native path:**

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

**WASM Component:**

```bash
# Build WASI Component (~1.7MB release)
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
│    │ wit::render()│         │ wasmtime Host    │    │
│    └──────┬───────┘         └────────┬─────────┘    │
│           │                          │               │
│           ▼                          ▼               │
│    DrawCmd (native)    WitDrawCmd (WASM boundary)    │
│           │                          │               │
│           └──────────┬───────────────┘               │
│                      ▼                               │
│              TinySkiaRenderer                        │
│              (Canvas 2D → pixels)                    │
└─────────────────────────────────────────────────────┘
```

## Build & Test

```bash
# Build
cargo build --workspace

# Test (180 tests)
cargo test --workspace

# Build WASM component
cargo build -p mermaid-canvas-wit-wasm --target wasm32-wasip2 --release

# Lint
cargo clippy --workspace
```

## License

Licensed under the [MIT License](LICENSE).

Copyright (c) 2026 StarEcho Pte. Ltd.
