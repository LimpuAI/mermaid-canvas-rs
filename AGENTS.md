# mermaid-canvas-rs — Agent 工作须知

Limpu 生态的 canvas 组件仓：Mermaid 图渲染组件，产出 WASI Component Model 组件并经 [aixpack](https://github.com/LimpuAI/aixpack) 打成标准插件包。对外文档不使用内部宿主代号（品牌纪律见 aixpack 仓 AGENTS.md）。

## 常用命令

```bash
cargo test --workspace                    # 单测
./scripts/verify-package.sh               # 插件包三查（aixpack：WIT 双侧/登记处/哈希三方）
aixpack --root . build && aixpack --root . pkg --zip   # 产包（dist/，不入库）
```

## 红线

1. **WIT 契约单源**：本仓 wit 是本组件协议的原身；同一文件以字节一致（mod EOL）存在于契约权威侧——两侧同 PR 演进，verify 查一机械把关（`CONTRACTS_ROOT` 指向契约仓或 aixpack checkout）。
2. **包由 aixpack 产**：plugin.toml 由工具按组件字节重生成（勿手改 dist/ 内清单）；哈希钉定与 sidecar 双写不可绕过。
3. **dist/ 不入库**；测试临时文件不写系统 temp。

## 打包身份

world `mermaid:viz@3.0.0` · 域键 `diagram-renderer` · 组件 crate `crates/mermaid-canvas-wit-wasm`（pack.toml 为准）。
