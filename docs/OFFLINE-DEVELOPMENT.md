# 离线开发流程

DM 支持把前端 npm 依赖和 Rust Cargo crates 放到项目内专用目录 `offline/`，用于无外网环境开发、构建和打包。

## 目录约定

| 目录 | 内容 |
|------|------|
| `offline/npm-cache/` | npm 离线缓存，来源于 `web/package-lock.json` |
| `offline/cargo/vendor/` | Cargo vendor crates，来源于 `Cargo.lock` |
| `offline/cargo/config.toml` | Cargo source replacement 配置片段 |
| `target/packages/` | 打包产物，不属于依赖缓存 |

## 在有网络的机器准备依赖

```bash
./scripts/prepare-offline-deps.sh
```

脚本会执行：

```bash
cd web && npm ci --cache ../offline/npm-cache --prefer-offline
cargo vendor offline/cargo/vendor > offline/cargo/config.toml
```

准备完成后，把整个项目目录复制到离线机器，必须包含：

```text
Cargo.lock
Cargo.toml
web/package-lock.json
web/package.json
offline/
```

## 离线机器前端安装

```bash
cd web
npm ci --offline --cache ../offline/npm-cache
npm run build
```

如果 npm 提示缓存缺包，说明有网络环境准备缓存时依赖没有完全下载，需要回到有网络机器重新执行：

```bash
./scripts/prepare-offline-deps.sh
```

## 离线机器 Cargo 构建

Cargo vendor 配置可以临时通过环境变量指定：

```bash
mkdir -p .cargo
cp offline/cargo/config.toml .cargo/config.toml
cargo build --release --target x86_64-unknown-linux-musl
```

也可以在 CI 或临时 shell 中使用：

```bash
CARGO_HOME="$PWD/offline/cargo-home" cargo build --offline --release --target x86_64-unknown-linux-musl
```

推荐使用 `.cargo/config.toml` 方式，因为 `cargo vendor` 生成的配置会把 crates.io 替换为 `offline/cargo/vendor`。

## 离线打包

当前项目不再要求 loongarch64 包。常用离线打包命令：

```bash
PACKAGE_TARGETS="x86_64-unknown-linux-musl aarch64-unknown-linux-musl" ./package.sh
```

如果离线机器没有 aarch64 交叉编译环境，只打当前 x86_64：

```bash
PACKAGE_TARGETS="x86_64-unknown-linux-musl" ./package.sh
```

## 注意事项

- 不要手工修改 `offline/cargo/vendor/` 下的依赖源码，升级依赖应修改 `Cargo.toml` 后重新生成 vendor。
- 不要把系统级工具链误认为项目依赖。Rust toolchain、Node.js、npm、musl-gcc、cross/Docker 仍需在离线机器提前安装或通过内网镜像提供。
- `node_modules/` 是安装结果，不建议作为源码依赖目录长期维护；离线缓存应以 `offline/npm-cache/` 为准。
- `target/` 是构建产物，可以删除后重建。
