#!/bin/bash
# DM 构建脚本: 单架构 / 多架构
# 用法:
#   ./build.sh                  # 默认: 当前平台
#   ./build.sh --target all     # 全架构 (需预装目标工具链)
#   ./build.sh --target x86_64-unknown-linux-musl aarch64-unknown-linux-musl ...
#   ./build.sh --no-frontend    # 跳过前端 (CI 复用已 build 的 dist)
#   ./build.sh --no-archive     # 不打包成 tar.gz
#
# 输出: dist/<target>/dm + dist/dm-<target>.tar.gz
set -e

cd "$(dirname "$0")"
ROOT="$(pwd)"
DIST="${ROOT}/dist"
mkdir -p "${DIST}"

TARGET="native"
BUILD_FRONTEND=1
ARCHIVE=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      shift
      if [[ "$1" == "all" ]]; then
        TARGET="all"
      else
        TARGET="$1"
      fi
      shift
      ;;
    --no-frontend) BUILD_FRONTEND=0; shift ;;
    --no-archive) ARCHIVE=0; shift ;;
    *) echo "unknown arg: $1"; exit 1 ;;
  esac
done

ALL_TARGETS=(
  "x86_64-unknown-linux-gnu"
  "x86_64-unknown-linux-musl"
  "aarch64-unknown-linux-gnu"
  "aarch64-unknown-linux-musl"
  "loongarch64-unknown-linux-gnu"
  "loongarch64-unknown-linux-musl"
  "mips-unknown-linux-musl"
  "mipsel-unknown-linux-musl"
  "sw_64-unknown-linux-gnu"
)

if [[ "${TARGET}" == "native" ]]; then
  HOST=$(rustc -vV | sed -n 's|host: ||p')
  TARGETS=("${HOST}")
elif [[ "${TARGET}" == "all" ]]; then
  TARGETS=("${ALL_TARGETS[@]}")
else
  TARGETS=("${TARGET}")
fi

# 1) 前端
if [[ ${BUILD_FRONTEND} -eq 1 ]]; then
  echo "[1/3] 构建前端..."
  if [[ -d offline/npm-cache ]]; then
    (cd web && npm ci --offline --cache ../offline/npm-cache --prefer-offline --no-audit --fund=false && npm run build)
  else
    (cd web && npm ci --no-audit --fund=false && npm run build)
  fi
fi

# 2) Rust 编译 (每个 target)
echo "[2/3] 编译 Rust..."
for t in "${TARGETS[@]}"; do
  echo "  -> ${t}"
  # 对 musl / 非原生 target 加 +crt-static (静态链接)
  extra=""
  if [[ "${t}" == *"-musl" ]]; then
    extra="--config=target.${t}.rustflags=-C+target-feature=+crt-static"
  fi
  # loongarch64 默认 rust 工具链支持从 1.78 起; 申威 sw_64 在 1.78+ tier 3
  # mips 需要 RUSTC_BOOTSTRAP=1 可能在旧 toolchain 不支持, 这里直接尝试
  set +e
  cargo build --release --target "${t}" ${extra} 2>&1 | tail -10
  rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then
    echo "  !! target ${t} 编译失败 (rc=${rc}), 跳过"
    continue
  fi
  out_dir="${DIST}/${t}"
  mkdir -p "${out_dir}"
  cp "target/${t}/release/dm" "${out_dir}/dm"
  # 写个 release.json
  cat > "${out_dir}/release.json" <<JSON
{
  "name": "dm",
  "version": "$(grep '^version' Cargo.toml | head -1 | sed 's|.*= ||;s|[\" ]||g')",
  "target": "${t}",
  "binary": "dm"
}
JSON
  echo "  ✓ ${out_dir}/dm"
done

# 3) 打包
if [[ ${ARCHIVE} -eq 1 ]]; then
  echo "[3/3] 打包..."
  for d in "${DIST}"/*/; do
    [[ -f "${d}dm" ]] || continue
    t=$(basename "${d}")
    out="${DIST}/dm-${t}.tar.gz"
    tar -C "${d}" -czf "${out}" dm
    echo "  ✓ ${out}"
  done
fi

# 4) 当前平台软链 ./dm
HOST=$(rustc -vV | sed -n 's|host: ||p')
if [[ -f "target/${HOST}/release/dm" ]]; then
  cp "target/${HOST}/release/dm" ./dm
  echo "=== 构建完成 (host=${HOST}) ==="
  ls -lh ./dm
  echo "使用: ./dm serve"
else
  echo "=== 当前平台未编译 (target=${HOST}), 检查 dist/ ==="
  ls -la "${DIST}/"
fi
