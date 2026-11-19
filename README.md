# PurePan

<div align="center">

![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)
![Rust](https://img.shields.io/badge/Rust-1.95%2B-orange.svg)
![GPUI](https://img.shields.io/badge/UI-GPUI%20%2B%20gpui--kit-blueviolet.svg)

🚀 基于 Rust 的高性能百度网盘客户端 | High-performance Baidu Netdisk client built with Rust

[功能特性](#-功能特性) • [快速开始](#-快速开始) • [技术栈](#️-技术栈) • [项目结构](#-项目结构) • [最新版本](#-最新版本) • [贡献](#-贡献)

</div>

---

## 📖 简介

PurePan 是一个使用 Rust 和 GPUI（gpui-kit 组件库）构建的现代化百度网盘第三方桌面客户端，提供简洁的界面和高效的上传/下载功能。

桌面端与后端运行在**同一进程**中：UI 通过进程内门面（`backend/src/server/inproc`）直接调用后端逻辑，不启动本地 HTTP 服务、不经过 WebSocket 或令牌认证。`purepan-backend` 仍可作为独立 HTTP/WebSocket 服务单独部署。

## ⭐ 支持项目发展

如果这个项目对你有帮助，请点一个 **Star** 支持！

<div align="left">

[![Star](https://img.shields.io/github/stars/d4go/purepan?style=social)](https://github.com/d4go/purepan/stargazers)

</div>

> 你的 Star 会让我更有动力持续更新 🚀

### 🎯 项目初衷

本项目主要是为了解决以下痛点：

- **高性能下载需求**：通过多线程并发下载、断点续传等技术，充分利用会员带宽，实现满速下载
- **自动备份功能**：支持将本地文件自动备份到百度网盘，实现数据的安全存储和同步
- **增强上传体验**：支持文件/文件夹上传、秒传能力与上传任务管理，让本地备份到网盘的过程更可控
- **跨平台支持**：支持 Windows、Linux、macOS 等多种平台，方便在不同设备上使用
- **现代化体验**：提供简洁美观的界面，支持实时进度显示和任务管理

### 🙏 致谢

本项目参考并受到了以下优秀项目的启发：
- [komorebiCarry/BaiduPCS-Rust](https://github.com/komorebiCarry/BaiduPCS-Rust) BaiduPCS-Go 的Rust版本，提供加密功能
- [qjfoidnh/BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go) - BaiduPCS-Go 的增强版本，提供分享链接/秒传链接转存功能
- [GangZhuo/BaiduPCS](https://github.com/GangZhuo/BaiduPCS) - 百度网盘命令行客户端

感谢这些项目的开源贡献，为本项目的开发提供了重要参考。

---

## ✨ 功能特性

### 👥 多账号与资源配额
- ✅ **多账号独立管理**：添加 / 切换 / 删除多个账号，每账号独立会话与任务队列；下载 / 上传 / 转存 / 备份 / 离线下载均按 `owner_uid` 隔离
- ✅ **实时资源配额**：机器总线程按会员权重在账号间分配，每账号展示「保底 / 上限」；自动按等级推荐（普通会员 10 / 超级会员 15）

### 🔐 认证系统
- ✅ 二维码扫码登录（百度网盘 APP 扫码）
- ✅ **Cookie 登录**：支持直接粘贴浏览器 DevTools 中复制的完整 Cookie 字符串完成登录
- ✅ 自动会话管理
- ✅ 会话持久化
- ✅ 自动登录状态验证
- ✅ 失效自动跳转登录页

### 📁 文件管理
- ✅ 浏览网盘文件和目录
- ✅ 目录导航（面包屑）
- ✅ 文件列表展示（表格视图）
- ✅ 文件信息展示（名称、大小、时间、类型）

### ⬇️ 下载引擎
- ✅ 单文件下载（多线程并发下载，8 个并发分片，可配置）
- ✅ **文件夹下载**（递归下载整个文件夹，自动扫描并创建任务）
- ✅ **批量下载**（支持多文件/文件夹同时下载）
- ✅ **下载冲突策略**（支持覆盖、跳过、自动重命名，适用于单文件/文件夹/批量下载）
- ✅ 断点续传支持
- ✅ 速度限制（可配置）
- ✅ 实时进度显示（下载速度、进度百分比、ETA）
- ✅ 任务队列管理
- ✅ 暂停/继续/删除功能
- ✅ URL 健康管理和智能淘汰策略
- ✅ **CDN 链接刷新三层检测机制**（速度异常检测、线程停滞检测、定时强制刷新）
- ✅ **下载文件资源管理器**（选择下载目录，支持最近目录记忆）

### ⬆️ 上传引擎与任务管理
- ✅ 统一的上传任务列表视图（上传管理页面）
- ✅ 支持选择本地文件或文件夹发起上传（通过本地文件资源管理器）
- ✅ **批量上传**（支持多文件/文件夹同时上传）
- ✅ **上传冲突策略**（支持智能去重、自动重命名、覆盖，可设置全局默认值或单次覆盖）
- ✅ **统一上传按钮**（文件和文件夹使用同一个上传入口）
- ✅ 上传任务进度实时展示：已上传大小、总大小、上传速度、剩余时间（ETA）
- ✅ 任务控制：暂停/继续/重试/删除
- ✅ 秒传标识：支持后端秒传时，在任务上展示"秒传"标记
- ✅ **上传最近目录记忆**（自动记录最近使用的上传目录）

### 🎨 客户端界面
- ✅ 基于 GPUI + gpui-kit 的原生桌面 UI
- ✅ 窗口自适应布局
- ✅ 实时状态更新
- ✅ 友好的用户体验

### 💻 本地文件资源管理器（上传文件选择器）
- ✅ 仿系统"资源管理器"的本地文件浏览体验
- ✅ 支持根目录、前进/后退、返回上一级、刷新等导航操作
- ✅ 支持文件/目录/文件或目录三种选择模式（根据上传场景配置）
- ✅ 支持分页加载和"加载更多"，适配大目录场景
- ✅ 提供空态/错误态 UI，操作失败可一键重试

### 🔗 转存功能
- ✅ **分享链接转存**（支持转存百度网盘分享链接到自己的网盘）
- ✅ 提取码验证（支持带提取码的分享链接）
- ✅ 转存后自动下载（可选，转存成功后自动创建下载任务）
- ✅ 转存任务管理（查看转存进度和状态）

### 🔁 分享同步（Share Sync）🆕
- ✅ **订阅分享链接**，周期/定时拉取并与上次快照对比，自动转存到网盘和/或下载到本地（仅同步变动文件）
- ✅ **多账号、双目标、可过滤**：按账号隔离，单条订阅可同时配「网盘 + 本地」，支持 include/exclude 路径
- ✅ **冲突策略**：覆盖 / 新版本 / 跳过；删除行为可控（默认保留旧文件，可选跟随分享者删除）
- ✅ **实时进度与审计**：子任务进度（速度/预计剩余）、运行历史（added/modified/removed/failed）

### ⬇️ 离线下载
- ✅ **离线下载功能**（支持通过百度网盘服务器代为下载资源到网盘空间）
- ✅ 多种链接格式支持：HTTP/HTTPS、磁力链接（magnet）、ed2k 链接
- ✅ 磁力链接自动标准化（Base32 转十六进制）
- ✅ 任务管理：添加、查询、取消、删除离线下载任务
- ✅ 实时进度显示：实时推送任务状态和进度更新（桌面端进程内订阅，独立后端模式为 WebSocket）
- ✅ **自动下载功能**：任务完成后可自动下载到本地，支持配置保存路径和每次询问路径选项
- ✅ 智能轮询机制：根据任务进度预测完成时间，动态调整轮询间隔（最小 3 分钟，最大 60 分钟）
- ✅ 任务详情查看：支持查看任务的完整信息，包括文件列表、保存路径、源链接等

### 🖥️ 运行模式
- ✅ 桌面端（GPUI）与后端同进程运行，进程内直接调用，无本地端口与令牌认证
- ✅ `purepan-backend` 可独立运行，提供 HTTP API + WebSocket 实时推送

### 📡 实时与持久化能力
- ✅ 任务持久化与断点恢复
- ✅ 实时事件推送（桌面端进程内订阅，独立后端模式为 WebSocket）
- ✅ 日志持久化与滚动

### 🌐 网络代理
- ✅ **HTTP 和 SOCKS5 代理支持**：支持配置代理服务器转发所有网络请求
- ✅ **代理认证**：支持用户名和密码认证
- ✅ **代理测试连接**：实时检测代理可用性和延迟
- ✅ **运行状态监控**：显示代理状态（正常/已回退到直连/探测中）、抖动次数、下次探测倒计时
- ✅ **自动回退机制**：代理故障时自动回退到直连模式，可配置是否允许回退
- ✅ **即时生效**：配置保存后立即生效，无需重启应用

### 🔄 自动备份
- ✅ **上传备份**（本地 → 云端）：自动将本地文件夹备份到百度网盘
- ✅ **下载备份**（云端 → 本地）：自动将云端文件同步到本地
- ✅ **文件系统监听**：实时检测本地文件变化（Windows 使用 ReadDirectoryChangesW）
- ✅ **定时轮询兜底**：防止监听遗漏，支持间隔轮询和指定时间全量扫描
- ✅ 备份配置管理：创建/编辑/删除配置、手动触发、禁用/启用
- ✅ 备份历史记录与 SQLite 持久化

### 🔐 客户端侧加密
- ✅ **AES-256-GCM 加密算法**：端到端加密保护文件隐私
- ✅ **支持普通上传和自动备份**：灵活选择是否启用加密
- ✅ 加密密钥管理：生成、导出、删除密钥
- ✅ 加密文件使用 `.dat` 扩展名隐藏真实文件类型
- ✅ 加密快照管理：记录映射关系，支持下载自动解密
- ✅ **文件管理原始文件名显示**：加密文件在文件列表中自动还原显示原始文件名

> ⚠️ **重要提示**：请务必妥善备份以下文件，否则已加密的文件将**无法解密**！
> - `config/encryption.json`：加密密钥文件（建议同时使用界面的"导出密钥"功能备份）
> - `config/baidu-pcs.db`：数据库文件，包含加密映射表（记录加密文件与原始文件名的对应关系）

### 🔓 独立解密工具 (decrypt-cli)

为了方便用户在没有主程序的情况下解密文件，我们提供了独立的命令行解密工具 `decrypt-cli`。

**特性：**
- ✅ 独立运行，无需启动主程序
- ✅ 支持批量解密和单文件解密
- ✅ 支持 AES-256-GCM 和 ChaCha20-Poly1305 算法
- ✅ 跨平台支持（Windows、Linux、macOS）

**下载：**
- 可从 [Releases 页面](https://github.com/d4go/purepan/releases) 查看发布的构建产物
- 也可以从源码构建：`cargo build --release -p decrypt-cli`

<details>
<summary>📖 <b>decrypt-cli 使用说明</b>（点击展开）</summary>

#### 准备工作

解密前需要准备以下文件：
1. `encryption.json` - 加密密钥文件（从主程序导出或备份）
2. `mapping.json` - 加密映射文件（批量解密时需要，可从主程序导出）
3. 加密的文件（`.dat` 文件）

#### 批量解密模式

```bash
# 恢复原始目录结构（使用映射文件中的原始路径）
decrypt-cli decrypt --key-file encryption.json --map mapping.json --in-dir ./encrypted --out-dir ./decrypted

# 镜像输入目录结构（按 --in-dir 的目录结构输出）
decrypt-cli decrypt --key-file encryption.json --map mapping.json --in-dir ./encrypted --out-dir ./decrypted --mirror
```

#### 单文件解密模式

```bash
# 自动尝试所有密钥
decrypt-cli decrypt --key-file encryption.json --in file.dat --out file.txt

# 指定密钥版本
decrypt-cli decrypt --key-file encryption.json --in file.dat --out file.txt --key-version 2
```

#### 参数说明

| 参数 | 说明 |
|------|------|
| `--key-file` | 密钥配置文件路径（必需） |
| `--map` | 映射文件路径（批量模式必需） |
| `--in-dir` | 输入目录（批量模式） |
| `--out-dir` | 输出目录（批量模式） |
| `--in` | 单个输入文件（单文件模式） |
| `--out` | 单个输出文件（单文件模式） |
| `--key-version` | 指定密钥版本（单文件模式可选） |
| `--mirror` | 镜像输入目录结构，而不是恢复原始路径 |

</details>

<details>
<summary>📖 <b>加密映射原理说明</b>（点击展开）</summary>

#### 文件加密
- 每个文件加密后生成唯一的 UUID 文件名（如 `a1b2c3d4-xxxx.dat`）
- 映射关系：`加密文件名 → 原始文件名` 存储在数据库中

#### 文件夹加密映射
文件夹使用**路径感知**的加密映射策略：

**加密时（原始名 → 加密名）**：根据 `(父路径, 文件夹名)` 查询或生成 UUID
```
示例：
1. 首次遇到 /backup/a/docs：
   - 查询：parent=/backup/a, name=docs → 未找到
   - 生成新 UUID：abc-123-xxx
   - 保存映射：(/backup/a, docs) → abc-123-xxx

2. 首次遇到 /backup/b/docs：
   - 查询：parent=/backup/b, name=docs → 未找到
   - 生成新 UUID：def-456-xxx（不同路径，不同 UUID）
   - 保存映射：(/backup/b, docs) → def-456-xxx

3. 再次遇到 /backup/a/docs：
   - 查询：parent=/backup/a, name=docs → 找到 abc-123-xxx
   - 复用已有加密名（保持一致性）
```

**解密时（加密名 → 原始名）**：直接根据 UUID 查询，无需父路径（UUID 全局唯一）

这种设计确保：
- ✅ 不同路径下的同名文件夹使用不同的加密名，避免冲突
- ✅ 同一路径下的文件夹多次上传时复用同一加密名，保持一致性
- ✅ 解密时通过唯一的 UUID 直接还原原始文件夹名

</details>

---

## 🚀 快速开始

<details>
<summary><b>🔐 登录流程说明</b>（点击展开）</summary>

#### 操作步骤

1. **打开百度网盘 APP**
    - 在手机上打开百度网盘 APP
    - 确保 APP 已登录你的账号

2. **扫描二维码**
    - 打开应用
    - 登录页会自动显示登录二维码
    - 打开百度网盘 APP，点击"扫一扫"功能
    - 扫描应用中显示的二维码

3. **确认登录**
    - 扫描成功后，APP 会弹出确认登录提示
    - **重要：在 APP 中点击确认登录**
    - **重要：确认后，APP 不能关闭或切换到后台**，否则登录会失败

4. **等待登录完成**
    - 应用会自动轮询登录状态（通常 1-2 秒）
    - 登录成功后会自动进入文件管理界面

#### ⚠️ 重要提示

- **APP 必须保持打开状态**：确认登录后，不要立即在 APP 中点击确认，先稍等片刻，等待应用显示"扫码成功"后，再按提示继续操作。
- APP 不能关闭、不能退出、不能切换到后台，必须保持在前台运行，直到应用显示登录成功
- **原因说明**：应用需要通过轮询机制查询登录状态，如果 APP 关闭，百度服务器可能无法正确返回登录状态，导致登录失败
- **登录失败处理**：如果等待一段时间后仍无法登录，请：
    1. 返回登录页重新获取二维码
    2. 重新执行上述操作步骤
    3. 确保 APP 在确认登录后保持打开状态

#### 登录状态保持

- 登录成功后，会话会自动保存到本地
- 下次启动应用时会自动恢复登录状态
- 如果会话过期，会自动跳转到登录页面重新登录

</details>

### 从源码构建

PurePan 使用 Cargo workspace 管理三个 crate：

- `purepan-desktop`：GPUI 桌面端（二进制 `purepan`），与后端同进程运行
- `purepan-backend`：后端核心库，也可作为独立 HTTP/WebSocket 服务运行
- `decrypt-cli`：独立解密工具

无需 Node.js / 浏览器运行时。

#### 前提条件

- Rust stable（>= 1.95，建议最新 stable，`rustup`；`backend/rust-toolchain.toml` 已指向 stable）
- 系统依赖：
  - **Linux**：`libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev libvulkan1`，以及可用的 Vulkan 驱动；图形会话（Wayland/X11）
  - **macOS**：Xcode Command Line Tools（macOS 15+）
  - **Windows**：Visual Studio 2022 C++ 生成工具 + CMake（MSVC 工具链）

#### 开发模式

```bash
cargo run -p purepan-desktop
```

#### 构建

```bash
# 构建整个 workspace
cargo build

# 仅桌面端（debug / release）
cargo build -p purepan-desktop
cargo build --release -p purepan-desktop

# 独立后端服务
cargo run -p purepan-backend
```

二进制位于 `target/{debug,release}/purepan`（Windows 为 `purepan.exe`），独立后端为 `purepan-backend`。

#### 数据目录

- **debug**：`desktop/`（crate 目录），配置位于 `desktop/config/app.toml`
- **release**：系统数据目录下的 `com.d4go.PurePan`（Linux 为 `~/.local/share/`，macOS 为 `~/Library/Application Support/`，Windows 为 `%APPDATA%`）
- 可用 `PUREPAN_DATA_DIR` 环境变量覆盖

> ⚠️ 数据目录中的 `accounts.json`、`session.json` 含百度账号凭据，已在 `.gitignore` 中忽略，请勿提交。

---
## 📝 平台支持说明

### 测试平台

- ✅ **macOS (Apple Silicon)** - 主要开发与测试环境
- ✅ **GitHub Actions CI** - Windows x64 / Linux x64 / Linux ARM64 / macOS Apple Silicon / macOS Intel

### 其他平台

虽然项目理论上支持 Linux、macOS、Windows 等平台，但部分平台未做主动测试。如果您在使用中遇到问题，欢迎提交 Issue。

### 多平台打包

项目使用 **GitHub Actions** 进行自动化多平台构建和打包，支持：

- Windows (x86_64)：NSIS 安装包（含应用图标、开始菜单快捷方式与卸载程序）
- Linux (x86_64, ARM64)：tar.gz 归档（含 `.desktop` 文件与图标）
- macOS (Apple Silicon, Intel)：`.app` 应用包（含 icns 图标，打包为 zip）

所有产物均内置应用图标。预编译安装包可在 [Releases](https://github.com/d4go/purepan/releases) 页面下载。

---

## 🛠️ 技术栈

### 后端（purepan-backend）

- **语言**: Rust 1.95+（stable）
- **Web 框架**: Axum 0.7（仅独立后端模式）
- **异步运行时**: Tokio
- **HTTP 客户端**: Reqwest
- **序列化**: Serde + Serde JSON
- **日志**: Tracing

### 桌面端（purepan-desktop）

- **UI**: GPUI + gpui-kit
- **调用方式**: 进程内直调（`backend/src/server/inproc`），无本地 HTTP / WebSocket 客户端
- **实时事件**: 直接订阅后端 `WsManager`
- **状态管理**: GPUI Global / Entity + UiEvent 事件泵

### 构建与发布

- **Cargo workspace**: backend / desktop / decrypt-cli
- **CI**: GitHub Actions 多平台构建与打包

---

<details>
<summary><b>📁 项目结构</b>（点击展开）</summary>

```
purepan/
├── backend/                     # 后端核心库 + 独立 HTTP/WebSocket 服务
│   ├── src/
│   │   ├── auth/                # 认证模块
│   │   ├── netdisk/             # 网盘 API
│   │   ├── downloader/          # 下载引擎
│   │   ├── uploader/            # 上传引擎
│   │   ├── transfer/            # 转存
│   │   ├── share_sync/          # 分享同步
│   │   ├── autobackup/          # 自动备份
│   │   ├── encryption/          # 客户端加密
│   │   ├── filesystem/          # 本地文件系统访问
│   │   ├── persistence/         # 任务持久化
│   │   ├── server/              # axum 路由 / AppState / 事件
│   │   │   ├── handlers/        # HTTP handlers
│   │   │   ├── inproc/          # 桌面端进程内直调门面
│   │   │   └── websocket/       # WS 协议 + 进程内事件订阅
│   │   └── server_setup.rs      # 启动引导（桌面端复用）
│   ├── rust-toolchain.toml
│   └── Cargo.toml
├── desktop/                     # GPUI 桌面端（二进制 purepan）
│   ├── src/
│   │   ├── api/                 # 进程内 API 封装
│   │   ├── views/               # 页面
│   │   ├── components/          # 通用组件
│   │   ├── state/               # 全局状态
│   │   ├── shell/               # 数据目录 / 单实例 / 托盘
│   │   ├── backend_embed.rs     # 后端引导与事件桥
│   │   └── main.rs
│   ├── config/                  # 开发环境配置与运行时数据（凭据文件已忽略）
│   └── assets/
├── decrypt-cli/                 # 独立解密 CLI
├── config/                      # 配置模板（app.toml.example 等）
├── scripts/                     # share-sync 诊断脚本
├── tests/                       # 集成 / 性能测试脚本
├── Cargo.toml                   # workspace 定义
├── rustfmt.toml                 # Rust 代码格式配置
├── .gitignore
├── LICENSE                      # Apache License 2.0
└── README.md
```

</details>

---

## ⚙️ 配置

配置文件位置：

- **桌面端（开发）**：`desktop/config/app.toml`
- **桌面端（release）**：系统数据目录下的 `config/app.toml`（见「数据目录」）
- **独立后端**：工作目录下的 `config/app.toml`

```toml
[server]
host = "127.0.0.1"
port = 18888
cors_origins = ["*"]

[download]
download_dir = "downloads"
max_global_threads = 10
max_concurrent_tasks = 2
chunk_size_mb = 10
max_retries = 3
```

> `[server]` 段仅在独立后端模式下生效；桌面端不监听端口。

<details>
<summary><b>📋 配置参数说明</b>（点击展开）</summary>

#### 下载配置参数

- **`max_global_threads`**: 全局最大线程数（所有下载任务共享的并发分片数）
- **`max_concurrent_tasks`**: 最大同时下载文件数
- **`chunk_size_mb`**: 每个分片的大小（单位: MB）
- **`max_retries`**: 下载失败后的最大重试次数

#### 普通用户配置建议

普通用户请将**全局最大线程数**（`max_global_threads`）和**最大同时下载数**（`max_concurrent_tasks`）都设置为 1。

```toml
[download]
max_global_threads = 1
max_concurrent_tasks = 1
```

**⚠️ 注意**：调大线程数只会在短时间内提升下载速度，且极易很快触发限速，导致几小时至几天内账号在各客户端都接近 0 速。

#### SVIP 用户配置建议

SVIP 用户建议**全局最大线程数**（`max_global_threads`）设置为 10 以上，根据实际带宽可调大，但不建议超过 20。可以配合**最大同时下载数**（`max_concurrent_tasks`）调整，**注意最大同时下载数越大不代表速度越快**。

```toml
[download]
max_global_threads = 10    # 建议 10-20
max_concurrent_tasks = 2    # 可根据需求调整
```

</details>

---

## 🔐 独立后端 Web 认证（可选）

仅适用于以 `purepan-backend` 独立运行 HTTP/WebSocket 服务的场景，用于保护 API 访问。GPUI 桌面端为进程内直调，不经过该认证。

### 配置文件

在独立后端的工作目录 `config/app.toml` 中配置 `[web_auth]` 段：

```toml
[web_auth]
# 是否启用 Web 访问认证
enabled = false

# 认证模式："none" | "password" | "totp" | "password_totp"
mode = "none"
```

### 认证模式说明

| 模式 | 说明 |
|------|------|
| `none` | 无认证（默认） |
| `password` | 仅密码认证 |
| `totp` | 仅 TOTP 双因素认证 |
| `password_totp` | 密码 + TOTP 双因素认证 |

### TOTP 与恢复码

- 启用 `totp` 或 `password_totp` 后，通过后端 `/api/v1/web-auth` 接口完成密码设置、TOTP 绑定与恢复码生成
- 使用 Google Authenticator、Microsoft Authenticator 等应用扫描二维码
- 每个恢复码只能使用一次，建议妥善保存

### 紧急恢复

如果因配置错误导致无法访问，可直接编辑 `config/app.toml`，将 `enabled` 设为 `false` 临时禁用认证：

```toml
[web_auth]
enabled = false
mode = "none"
```

重启后端后即可无认证访问。

---

## 🧪 测试

```bash
# 全部单元测试
cargo test --workspace

# 仅后端
cargo test -p purepan-backend

# 集成 / 性能测试脚本
./tests/integration_test.sh
./tests/performance_test.sh
```

`scripts/` 下另有 share-sync 诊断脚本（Python），用于排查订阅、运行记录与快照状态。

---

## 📈 性能参考

| 指标 | 参考值 |
|------|--------|
| 应用启动时间 | 5-10 秒 |
| 内存占用（空闲） | 100-200MB |
| 内存占用（下载） | 200-500MB |
| 并发下载分片 | 最多 16 个 |

> 数值随平台、网络环境与任务规模变化，仅供参考。

---

## 🗺️ 路线图

- [x] ✅ 基础架构
- [x] ✅ 认证模块（二维码登录）
- [x] ✅ 文件浏览
- [x] ✅ 下载引擎（多线程 + 断点续传）
- [x] ✅ Web 服务器（RESTful API，独立后端模式）
- [x] ✅ GPUI 桌面端界面（gpui-kit）
- [x] ✅ 文件夹下载（目录下载）
- [x] ✅ 批量下载（多文件选择下载）
- [x] ✅ 批量上传（多文件/文件夹上传）
- [x] ✅ 上传功能
- [x] ✅ 百度网盘新建文件夹功能（⚠️ 可能需要重新登入才可使用）
- [x] ✅ CDN链接刷新三层检测机制（速度异常检测、线程停滞检测、定时强制刷新）
- [x] ✅ 转存分享链接功能
- [x] ✅ 上传下载最近目录记忆
- [x] ✅ 下载文件资源管理器
- [x] ✅ 任务持久化
- [x] ✅ 实时事件推送（桌面端进程内订阅 / 独立后端 WebSocket）
- [x] ✅ 日志持久化与滚动
- [x] ✅ 自动备份
- [x] ✅ 客户端侧加密（支持普通上传和自动备份）
- [x] ✅ 独立后端 Web 访问认证（密码保护 + TOTP 双因素认证）
- [x] ✅ 离线下载功能（支持 HTTP/HTTPS/磁力链接/ed2k，自动下载，智能轮询）
- [x] ✅ 网络代理支持（HTTP/SOCKS5 代理，支持认证、测试连接、运行状态监控、自动回退）
- [x] ✅ 批量任务操作（下载/上传任务批量暂停/继续/清除）
- [x] ✅ 桌面端进程内直调后端（无本地 HTTP / WebSocket 客户端依赖）
- [ ] 📝 下载/上传 SSD 缓冲区（针对 NAS 用户：① 减少 HDD 活跃时间，让硬盘更多休眠；② 提升传输效率，仅网速 > 磁盘速度时有效）

---

## 🤝 贡献

欢迎贡献代码！请遵循以下步骤：

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改（提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)，例如 `git commit -m 'feat: add some amazing feature'`）
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

### 开发规范

- **Rust**: 遵循 Rust 官方代码风格（使用 `cargo fmt` 和 `cargo clippy`）
- **UI**: GPUI / gpui-kit 改动需在真实窗口中验证交互
- **提交信息**: 遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范（`feat`/`fix`/`docs`/`chore` 等）；PR 标题与每个 commit 都会被 CI 校验

---

## 📄 许可证

本项目采用 Apache License 2.0 许可证 - 详见 [LICENSE](LICENSE) 文件

---

## ⚠️ 免责声明

本项目仅供学习和研究使用，请勿用于商业用途。使用本工具产生的任何问题与本项目无关。

---

## 🙏 致谢
- [komorebiCarry/BaiduPCS-Rust](https://github.com/komorebiCarry/BaiduPCS-Rust) - 本项目的重要参考
- [qjfoidnh/BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go) - 本项目的重要参考
- [GangZhuo/BaiduPCS](https://github.com/GangZhuo/BaiduPCS) - 原始项目灵感来源
- [Rust 社区](https://www.rust-lang.org/)
- [GPUI](https://gpui.rs/)
- [gpui-kit](https://gpui-kit.com/)
---

<div align="center">

**⭐ 如果这个项目对你有帮助，请给一个 Star！**

Made with ❤️ by Rust + GPUI

</div>
