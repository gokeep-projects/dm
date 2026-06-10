#!/bin/bash
set -e
echo "=== DM 构建脚本 ==="
echo "[1/3] 构建前端..."
if [ -d offline/npm-cache ]; then
  (cd web && npm ci --offline --cache ../offline/npm-cache --prefer-offline --no-audit --fund=false && npm run build)
else
  (cd web && npm ci --no-audit --fund=false && npm run build)
fi
echo "[2/3] 编译 Rust..."
cargo build --release
cp target/release/dm ./dm
echo "[3/3] 完成"
echo "=== 构建完成 ==="
ls -lh ./dm
echo "使用: ./dm serve"
