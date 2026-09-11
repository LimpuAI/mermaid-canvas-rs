#!/usr/bin/env bash
# install-hooks.sh —— 安装本地 git pre-commit 钩子：提交时跑 scripts/verify-package.sh
#（快路径：只做三查，不触发组件构建）。一次性安装；卸载即删 .git/hooks/pre-commit。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOK="$ROOT/.git/hooks/pre-commit"
{
  echo '#!/usr/bin/env bash —— 由 scripts/install-hooks.sh 生成（pack verify 门禁）'
  echo 'exec "$(dirname "$0")/../../scripts/verify-package.sh"'
} > "$HOOK"
chmod +x "$HOOK"
echo "install-hooks: pre-commit → scripts/verify-package.sh（$HOOK）"
