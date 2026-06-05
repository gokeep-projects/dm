#!/bin/bash
set -euo pipefail

echo "=== DM Linux musl 静态构建打包脚本 ==="
echo ""

rm -rf target/packages
mkdir -p target/packages

echo "[1/5] 构建前端..."
(cd web && npm run build)
echo "[OK] 前端构建完成"
echo ""

TARGETS=(
  "x86_64-unknown-linux-musl"
  "aarch64-unknown-linux-musl"
)

target_dir_for() {
  local target="$1"
  if [ "$target" = "aarch64-unknown-linux-musl" ]; then
    echo "target/cross-aarch64"
  else
    echo "target"
  fi
}

echo "[2/5] 检查 Rust musl 目标..."
for target in "${TARGETS[@]}"; do
  if ! rustup target list --installed | grep -q "^${target}$"; then
    echo "安装目标: $target"
    rustup target add "$target"
  fi
done
echo "[OK] 工具链检查完成"
echo ""

build_target() {
  local target="$1"
  echo "  构建 $target..."
  local static_flags="-C target-feature=+crt-static ${RUSTFLAGS:-}"
  if [ "$target" = "x86_64-unknown-linux-musl" ]; then
    RUSTFLAGS="$static_flags" \
      CC_x86_64_unknown_linux_musl="${CC_x86_64_unknown_linux_musl:-musl-gcc}" \
      cargo build --release --target "$target"
  elif command -v cross >/dev/null 2>&1; then
    if command -v docker >/dev/null 2>&1 && ! docker info >/dev/null 2>&1; then
      echo "    Docker daemon 未运行，尝试启动..."
      systemctl start docker 2>/dev/null || service docker start 2>/dev/null || true
    fi
    if command -v docker >/dev/null 2>&1 && ! docker info >/dev/null 2>&1; then
      echo "    [FAIL] cross 需要 Docker daemon，当前无法连接 /var/run/docker.sock"
      return 1
    fi
    RUSTFLAGS="$static_flags" \
      CARGO_TARGET_DIR="$(target_dir_for "$target")" \
      cross build --release --target "$target"
  else
    RUSTFLAGS="$static_flags" cargo build --release --target "$target"
  fi
}

echo "[3/5] 构建 Linux musl 静态二进制..."
if [ "${SKIP_RUST_BUILD:-0}" = "1" ]; then
  echo "  跳过 Rust 编译，使用 target/<target>/release/dm 既有产物"
else
  for target in "${TARGETS[@]}"; do
    if build_target "$target"; then
      echo "    [OK] $target"
    else
      echo "    [FAIL] $target 构建失败"
      exit 1
    fi
  done
fi
echo ""

echo "[4/5] 校验并打包..."
for target in "${TARGETS[@]}"; do
  binary="$(target_dir_for "$target")/$target/release/dm"
  if [ ! -f "$binary" ]; then
    echo "[FAIL] 缺少二进制: $binary"
    exit 1
  fi

  if command -v ldd >/dev/null 2>&1; then
    if ldd "$binary" 2>&1 | grep -v "not a dynamic executable" | grep -q "=>"; then
      echo "[FAIL] $target 二进制存在动态库依赖"
      ldd "$binary" || true
      exit 1
    fi
  fi

  package_name="dm-$target"
  package_dir="target/packages/$package_name"
  mkdir -p "$package_dir"
  cp "$binary" "$package_dir/"
  cp -r scripts "$package_dir/"
  cp scripts/install.sh "$package_dir/"
  cp scripts/uninstall.sh "$package_dir/"
  cat > "$package_dir/README.md" << 'EOF'
# DM 现场维护工具

本包为 Linux musl 静态二进制，无 glibc 等系统动态库依赖。

## 安装

```bash
sudo bash install.sh
```

## 使用

```bash
dm --help
dm list
dm serve --bind 0.0.0.0 --port 3399
```

## 卸载

```bash
sudo bash uninstall.sh
```
EOF

  (cd target/packages && zip -qr "$package_name.zip" "$package_name")
  rm -rf "$package_dir"
  echo "    [OK] target/packages/$package_name.zip"
done
echo ""

echo "[5/5] 构建完成"
echo ""
ls -lh target/packages/*.zip
