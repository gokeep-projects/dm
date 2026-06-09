#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OFFLINE_DIR="$ROOT/offline"
USE_OFFLINE_DEPS="${USE_OFFLINE_DEPS:-auto}"

echo "=== DM Linux musl 静态构建打包脚本 ==="
echo ""

rm -rf target/packages
mkdir -p target/packages

require_offline_deps() {
  if [ "$USE_OFFLINE_DEPS" = "auto" ]; then
    if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
      USE_OFFLINE_DEPS=0
      echo "[INFO] GitHub Actions 环境，自动联网下载依赖"
    elif [ -d "$OFFLINE_DIR/npm-cache" ] && [ -d "$OFFLINE_DIR/cargo/vendor" ]; then
      USE_OFFLINE_DEPS=1
      echo "[INFO] 检测到项目内 offline 依赖，优先使用本地离线依赖"
    else
      USE_OFFLINE_DEPS=0
      echo "[WARN] 未检测到完整 offline 依赖，改用联网下载依赖"
    fi
  fi

  if [ "$USE_OFFLINE_DEPS" != "1" ]; then
    echo "[INFO] USE_OFFLINE_DEPS=0，npm/Cargo 将联网下载依赖"
    return
  fi
  if [ ! -d "$OFFLINE_DIR/npm-cache" ]; then
    echo "[FAIL] 缺少 npm 离线缓存: $OFFLINE_DIR/npm-cache"
    echo "       请先执行: ./scripts/prepare-offline-deps.sh"
    exit 1
  fi
  if [ ! -d "$OFFLINE_DIR/cargo/vendor" ]; then
    echo "[FAIL] 缺少 Cargo vendor 目录: $OFFLINE_DIR/cargo/vendor"
    echo "       请先执行: ./scripts/prepare-offline-deps.sh"
    exit 1
  fi
}

run_npm() {
  if [ "$USE_OFFLINE_DEPS" = "1" ]; then
    (cd "$ROOT/web" && \
      npm ci --offline --cache "$OFFLINE_DIR/npm-cache" --prefer-offline --no-audit --fund=false && \
      npm run build)
  else
    (cd "$ROOT/web" && npm ci --no-audit --fund=false && npm run build)
  fi
}

cargo_base_cmd() {
  local cargo_toolchain_arg=()
  if [ "${1:-}" != "" ] && [[ "$1" == +* ]]; then
    cargo_toolchain_arg=("$1")
    shift
  fi
  if [ "$USE_OFFLINE_DEPS" = "1" ]; then
    cargo "${cargo_toolchain_arg[@]}" "$@"
  else
    cargo "${cargo_toolchain_arg[@]}" \
      --config 'net.offline=false' \
      --config 'source.crates-io.replace-with="crates-io"' \
      "$@"
  fi
}

require_offline_deps

echo "[1/5] 构建前端..."
run_npm
echo "[OK] 前端构建完成"
echo ""

DEFAULT_TARGETS=(
  "x86_64-unknown-linux-musl"
  "aarch64-unknown-linux-musl"
  "loongarch64-unknown-linux-musl"
  "mips-unknown-linux-musl"
  "mipsel-unknown-linux-musl"
  "mips64-unknown-linux-muslabi64"
  "mips64el-unknown-linux-muslabi64"
)

if [ -n "${PACKAGE_TARGETS:-}" ]; then
  # shellcheck disable=SC2206
  TARGETS=(${PACKAGE_TARGETS})
else
  TARGETS=("${DEFAULT_TARGETS[@]}")
fi

target_dir_for() {
  local target="$1"
  if [ "$target" = "aarch64-unknown-linux-musl" ]; then
    echo "target/cross-aarch64"
  elif [ "$target" != "x86_64-unknown-linux-musl" ]; then
    echo "target/cross-$target"
  else
    echo "target"
  fi
}

rustup_knows_target() {
  local target="$1"
  rustup target list | awk '{print $1}' | grep -qx "$target"
}

target_needs_build_std() {
  local target="$1"
  ! rustup_knows_target "$target"
}

echo "[2/5] 检查 Rust musl 目标..."
for target in "${TARGETS[@]}"; do
  if ! rustc --print target-list | grep -q "^${target}$"; then
    echo "[FAIL] 当前 rustc 不支持目标: $target"
    exit 1
  fi

  if ! rustup target list --installed | grep -q "^${target}$"; then
    if rustup_knows_target "$target"; then
      echo "安装目标: $target"
      rustup target add "$target"
    elif target_needs_build_std "$target"; then
      echo "目标 $target 未提供预编译 rust-std，使用 nightly build-std 构建"
      rustup toolchain install nightly --component rust-src
    else
      echo "[FAIL] 目标 $target 未提供预编译 rust-std，请设置 BUILD_STD=1 并安装可用交叉链接器"
      exit 1
    fi
  fi
done
echo "[OK] 工具链检查完成"
echo ""

build_target() {
  local target="$1"
  echo "  构建 $target..."
  local static_flags="-C target-feature=+crt-static ${RUSTFLAGS:-}"
  local build_std_args=()
  local cargo_toolchain=()
  local host_offline_env=()
  local cross_offline_env=()

  if [ "$USE_OFFLINE_DEPS" = "1" ]; then
    host_offline_env=(CARGO_NET_OFFLINE=true)
    cross_offline_env=(CARGO_NET_OFFLINE=true)
  fi

  if [ "${BUILD_STD:-0}" = "1" ] || target_needs_build_std "$target"; then
    build_std_args=(-Z build-std=std,panic_abort)
    cargo_toolchain=(+nightly)
  fi

  if [ "$target" = "x86_64-unknown-linux-musl" ]; then
    (
      export RUSTFLAGS="$static_flags"
      export CC_x86_64_unknown_linux_musl="${CC_x86_64_unknown_linux_musl:-musl-gcc}"
      if [ "$USE_OFFLINE_DEPS" = "1" ]; then
        export CARGO_NET_OFFLINE=true
      fi
      cargo_base_cmd "${cargo_toolchain[@]}" build --release --target "$target" "${build_std_args[@]}"
    )
  elif [ "${BUILD_WITH_ZIG:-0}" = "1" ] && command -v cargo-zigbuild >/dev/null 2>&1; then
    (
      export RUSTFLAGS="$static_flags"
      export CARGO_TARGET_DIR="$(target_dir_for "$target")"
      if [ "$USE_OFFLINE_DEPS" = "1" ]; then
        export CARGO_NET_OFFLINE=true
      fi
      cargo_base_cmd "${cargo_toolchain[@]}" zigbuild --release --target "$target" "${build_std_args[@]}"
    )
  elif command -v cross >/dev/null 2>&1; then
    if command -v docker >/dev/null 2>&1 && ! docker info >/dev/null 2>&1; then
      echo "    Docker daemon 未运行，尝试启动..."
      systemctl start docker 2>/dev/null || service docker start 2>/dev/null || true
    fi
    if command -v docker >/dev/null 2>&1 && ! docker info >/dev/null 2>&1; then
      echo "    [FAIL] cross 需要 Docker daemon，当前无法连接 /var/run/docker.sock"
      return 1
    fi
    env "${cross_offline_env[@]}" \
      RUSTFLAGS="$static_flags" \
      CARGO_TARGET_DIR="$(target_dir_for "$target")" \
      cross "${cargo_toolchain[@]}" build --release --target "$target" "${build_std_args[@]}"
  else
    (
      export RUSTFLAGS="$static_flags"
      export CARGO_TARGET_DIR="$(target_dir_for "$target")"
      if [ "$USE_OFFLINE_DEPS" = "1" ]; then
        export CARGO_NET_OFFLINE=true
      fi
      cargo_base_cmd "${cargo_toolchain[@]}" build --release --target "$target" "${build_std_args[@]}"
    )
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
