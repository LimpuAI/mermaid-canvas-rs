#!/usr/bin/env bash
# verify-package.sh —— 本仓标准插件包的机械校验（aixpack verify 三查；
# 契约见 https://github.com/LimpuAI/aixpack docs/plugin-package.md）。提交前/CI 位运行，漂移即红。
#
# aixpack 解析顺序（工具链是安装件，不是仓内依赖）：
#   1. PATH 上的 aixpack（cargo install --git https://github.com/LimpuAI/aixpack）
#   2. 兜底：兄弟 aixpack 仓的构建产物（AIXPACK_ROOT 覆写）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if command -v aixpack >/dev/null 2>&1; then
  CP="$(command -v aixpack)"
else
  AIX="${AIXPACK_ROOT:-$ROOT/../aixpack}"
  CP="$AIX/target/debug/aixpack"
  [ -x "$CP" ] || CP="$AIX/target/release/aixpack"
  if [ ! -x "$CP" ]; then
    echo "verify-package: aixpack 未安装——cargo install --git https://github.com/LimpuAI/aixpack，或在 AIXPACK_ROOT 指向的 checkout 内 cargo build" >&2
    exit 2
  fi
fi
REGISTRY="${REGISTRY_ROOT:-$ROOT/../Antimass}"
CONTRACTS="${CONTRACTS_ROOT:-${ECHODAWN_ROOT:-$ROOT/../echodawn-rs}}"
exec "$CP" --root "$ROOT" verify --wit-root crates/mermaid-canvas-wit/wit   --contracts-root "$CONTRACTS" ${REGISTRY:+--registry-root "$REGISTRY"}
