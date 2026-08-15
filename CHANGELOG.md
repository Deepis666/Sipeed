# Changelog

All notable changes to the Sipeed project will be documented in this file.

## [Unreleased]

### 2026-06-15 (Initial Setup & Configuration)

#### Added

- **opencode 多 agent 协同工作流**
  - `opencode.json` - 配置了 6 个 agent：`build`（主构建 agent）、`plan`（规划 agent，只读）、`test-runner`（测试执行）、`code-reviewer`（代码审查）、`code-fixer`（修复代码）、`log-keeper`（日志记录）
  - `.opencode/agents/code-fixer.md` - Code Fixer Agent 工作流定义
  - `.opencode/agents/code-reviewer.md` - Code Reviewer Agent 工作流定义
  - `.opencode/agents/log-keeper.md` - Log Keeper Agent 工作流定义
  - `.opencode/agents/test-runner.md` - Test Runner Agent 工作流定义
  - `.opencode/skills/game-launcher-ui/SKILL.md` - 游戏启动器 UI 样式技能
  - `Agent.md` - 项目级 Agent 角色定义（Game Launcher Developer）
  - `skills.md` - 游戏启动器 Modern UI 风格规范

- **测试框架**
  - `vitest` ^4.1.9 - 前端单元测试运行器
  - `@vue/test-utils` ^2.4.11 - Vue 3 组件测试工具
  - `jsdom` ^29.1.1 - DOM 模拟环境
  - `vitest.config.ts` - Vitest 配置文件（支持 `@/` alias、jsdom 环境、Vue 插件）
  - `package.json` 新增脚本：
    - `"test:unit": "vitest run"` - 运行单元测试
    - `"test:watch": "vitest"` - 监听模式测试
    - `"typecheck": "vue-tsc --noEmit"` - TypeScript/Vue 类型检查

- **依赖**
  - `@tanstack/vue-table` ^8.21.3 - Vue 表格组件（用于 shadcn-vue 的 DataTable）
  - `tailwind-merge` ^3.6.0 - Tailwind 类名合并工具
  - `clsx` ^1.2.1 - 条件类名工具

- **工具函数**
  - `src/lib/utils.ts` - cn() 和 valueUpdater() 工具函数

#### Fixed

- **[fix]** `src-tauri/src/lib.rs:62` - `ScanProgress` 结构体添加 `Clone` derive，修复 `spawn_blocking` 闭包中 `app_handle.clone()` 与 `ScanProgress` 的生命周期编译错误
- **[fix]** `package.json` - 添加 `@tanstack/vue-table` 依赖，修复 `src/lib/utils.ts` 中 `Updater` 类型导入的编译错误
- **[fix]** `src/App.vue` - 移除默认 Tauri 模板中的 `greet` 命令调用，清理未使用的模板代码（greet input/button 及相关样式）

#### Changed

- `tsconfig.json` - 添加 `vitest/globals` 类型支持和 `@/*` 路径别名映射
- `src/main.ts` - 添加全局样式导入 `import './style.css'`

### 2026-06-15 (Testing & Code Quality)

#### Added

- **前端单元测试**
  - `src/lib/__tests__/utils.test.ts` - 11 个测试用例
    - `cn()` 7 个：单个类名、合并多个类名、Tailwind 冲突解析（last wins）、falsy 值过滤、条件对象、嵌套数组、空输入
    - `valueUpdater()` 4 个：直接赋值、函数更新器、字符串值、对象值

- **Rust 单元测试**
  - `src-tauri/src/lib.rs` `#[cfg(test)]` 模块 - 17 个测试用例，覆盖：
    - 纯函数逻辑
    - 游戏候选人（game candidates）查询
    - 数据库操作

#### Fixed

- **[fix]** `src-tauri/src/lib.rs` - 合并 Windows/macOS 相同分支，修复 `clippy::if_same_then_else` 警告

#### Changed

- **工具链** - 安装 clippy（nightly 工具链），启用 Rust 代码静态检查

#### Verified

- **28 个测试全通过**（11 前端 + 17 Rust）
- **TypeCheck 通过**（`vue-tsc --noEmit`）
- **Clippy 0 警告**

