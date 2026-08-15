# Sipeed

轻量级 PC 游戏启动器，基于 **Tauri 2 (Rust) + Vue 3 + TypeScript** 构建的原生桌面应用。

- 纯原生桌面应用，不依赖外部浏览器
- 打包体积小（release 启用 `lto` + `opt-level=z` + `strip`）
- 暗色极简现代 UI（参考 Sofast / Raycast / Playnite Modern UI）

## 功能特性

- 🎮 **游戏库管理**：扫描本地游戏目录，自动识别可执行文件（`.exe`）
- 🕹️ **游戏启动**：一键启动游戏，支持游玩时长追踪（`process_state` 进程监控）
- 🖼️ **元数据获取**：GameBrain 游戏信息 + SteamGridDB 封面图
- 🗂️ **分类与搜索**：自定义分类、搜索、排序、网格/列表视图
- ⚙️ **设置面板**：音乐开关、AI 开关等

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust + Tauri v2 |
| 前端 | Vue 3 + TypeScript + Vite 6 |
| UI | Tailwind CSS + shadcn/vue 风格组件 + lucide 图标 |
| 数据库 | SQLite（tauri-plugin-sql + rusqlite） |
| 网络 | reqwest (Rust) |
| 测试 | Vitest + @vue/test-utils |

## 环境要求

| 依赖 | 版本 | 说明 |
|---|---|---|
| Node.js | ≥ 18（建议 20+） | 前端构建 |
| Rust | **nightly** | 由 `rust-toolchain.toml` 自动锁定，无需手动指定 |
| Tauri CLI | v2 | 作为 devDependency 已内置，无需全局安装 |
| 平台依赖 | Windows: WebView2（Win10/11 自带）<br/>Linux: `libwebkit2gtk-4.1-dev` 等<br/>macOS: Xcode CLT | 参考 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/) |

> Linux 用户额外需要系统包：`libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`

## 本地开发

### 1. 安装依赖

```bash
npm install
```

首次会同时安装前端依赖；Rust 依赖在首次编译时自动下载。

### 2. 配置 API 密钥（必需）

应用在 **Rust 编译时** 通过 `src-tauri/build.rs` 从环境变量注入密钥，Tauri CLI 会自动加载项目根目录的 `.env` 文件：

```bash
cp .env.example .env
```

编辑 `.env`，填入你自己的密钥：

```
GAMEBRAIN_API_KEY=your_key_here     # 免费申请：https://gamebrain.co/api
STEAMGRID_API_KEY=your_key_here     # 免费申请：https://www.steamgriddb.com/profile/preferences/api
```

> ⚠️ `.env` 已被 `.gitignore` 忽略，不会提交到仓库。密钥修改后需要**重新运行** `npm run tauri dev` 才会生效（编译期注入）。

### 3. 启动开发模式

```bash
npm run tauri dev
```

- 前端 dev server：`http://localhost:1420`（Vite，热更新）
- Rust 端代码修改会自动触发重新编译

### 常用脚本

| 命令 | 说明 |
|---|---|
| `npm run dev` | 仅启动前端 Vite（不带 Tauri 壳） |
| `npm run tauri dev` | 启动完整桌面应用（开发模式） |
| `npm run build` | 前端类型检查 + 生产构建（输出到 `dist/`） |
| `npm run tauri build` | 打包桌面安装包（输出到 `src-tauri/target/release/bundle/`） |
| `npm run test:unit` | 运行前端单元测试（Vitest） |
| `npm run typecheck` | TypeScript/Vue 类型检查 |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Rust 单元测试 |

## 项目结构

```
├── src/                    # 前端源码
│   ├── pages/              # 页面（HomePage / GameLibrary / GameDetail / SettingsPage）
│   ├── components/         # 组件（Sidebar / GameCard / SearchBar / ContextMenu）
│   ├── composables/        # 组合式函数（useTauri）
│   ├── lib/                # 工具函数 + 单元测试
│   ├── router/             # 路由
│   └── App.vue             # 根组件
├── src-tauri/              # Rust 后端
│   ├── src/lib.rs          # 核心逻辑（扫描、元数据、启动、时长追踪）
│   ├── src/main.rs         # 入口
│   ├── build.rs            # 编译期注入 API 密钥
│   ├── capabilities/       # Tauri 权限配置
│   └── tauri.conf.json     # 应用/打包配置
├── gamebrain/              # GameBrain 生成 SDK（参考用，未在 Cargo.toml 中启用）
├── .env.example            # 环境变量模板
├── Agent.md                # AI 辅助开发角色定义
└── skills.md               # UI 风格规范
```

## 打包发布

```bash
npm run tauri build
```

安装包输出位置：
- Windows: `src-tauri/target/release/bundle/nsis/*.exe`
- macOS: `src-tauri/target/release/bundle/dmg/`
- Linux: `src-tauri/target/release/bundle/deb/` 等

> 安装包体积约 2.8 MB（release 优化已开启）。

## 扩展开发指南

### 新增一个页面

1. 在 `src/pages/` 新建 `MyPage.vue`
2. 在 `src/router/index.ts` 注册路由
3. 在 `src/components/Sidebar.vue` 添加导航入口

### 新增一个 Rust 命令（供前端调用）

1. 在 `src-tauri/src/lib.rs` 添加 `#[tauri::command]` 函数
2. 在 `invoke_handler` 中注册（注意依赖注入的 state）
3. 前端通过 `@tauri-apps/api` 的 `invoke` 调用

### 新增 API 密钥

1. 在 `.env` 添加 `YOUR_SERVICE_API_KEY=xxx`
2. 在 `src-tauri/build.rs` 读取并注入为常量
3. 在 `src-tauri/src/lib.rs` 使用（参考现有 `GAMEBRAIN_KEY` 的用法）
4. 同步更新 `.env.example` 和本文档

## 贡献

欢迎参与协作开发！请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，了解分支规范、提交规范与代码审查要求。

## 许可

[MIT](LICENSE)
