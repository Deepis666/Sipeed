# 贡献指南

感谢你对 Sipeed 的关注！以下是参与协作开发的规范，请先阅读后再动手。

## 快速开始

```bash
# 1. fork 本仓库并 clone 到本地
git clone https://github.com/<你的用户名>/Sipeed.git
cd Sipeed

# 2. 安装依赖
npm install

# 3. 配置 API 密钥（见 README「配置 API 密钥」一节）
cp .env.example .env

# 4. 运行开发模式
npm run tauri dev
```

## 分支规范

- **主分支 `main`**：受保护，只接受通过 PR 合并，禁止直接 push
- 每次开发从最新的 `main` 拉出功能分支：`feat/xxx`、`fix/xxx`、`docs/xxx`、`refactor/xxx`

```bash
git checkout main
git pull
git checkout -b feat/my-feature
```

## 提交规范

使用 [Conventional Commits](https://www.conventionalcommits.org/) 格式：

```
<type>(<scope>): <描述>

# 示例
feat(game-library): 添加按平台筛选功能
fix(sidebar): 修复导航项高亮状态错误
docs(readme): 补充 Linux 环境依赖说明
refactor(scan): 重构目录扫描逻辑
test(utils): 为 cn() 添加边界用例
```

常用 type：`feat` / `fix` / `docs` / `refactor` / `test` / `chore` / `perf` / `style`

> scope 可选，指向受影响模块（如 `pages`、`components`、`src-tauri`）。

## 开发要求

### 提交前必须通过

```bash
npm run typecheck      # TypeScript/Vue 类型检查
npm run test:unit      # 前端单元测试
cargo test --manifest-path src-tauri/Cargo.toml   # Rust 测试（如涉及 Rust 改动）
```

### 规范

- **TypeScript**：严格模式，不引入 `any`（除非有充分理由并注释说明）
- **Rust**：遵循 clippy 建议，错误处理使用 `Result<T, E>`，避免 `unwrap()`（除非有注释说明为何安全）
- **样式**：使用 Tailwind CSS，遵循 `skills.md` 中的 UI 规范；新组件命名与现有 `src/components/` 保持一致
- **数据库**：SQLite 表结构变更需在 CHANGELOG 中记录
- **密钥**：任何 API 密钥只放入 `.env`（已 gitignore），**严禁**硬编码进代码或提交到仓库

### 新功能建议带上测试

- 前端工具函数：在 `src/lib/__tests__/` 添加 Vitest 用例
- 复杂 Rust 逻辑：在 `src-tauri/src/` 添加 `#[cfg(test)]` 模块

## PR 流程

1. 确保分支基于最新 `main`，已 rebase 无冲突
2. 提交说明清晰，一个 PR 只做一件事
3. 创建 PR 时使用仓库提供的模板（描述改动 + 验证方式 + 关联 issue）
4. CI（typecheck + 单元测试）通过后，等待 reviewer 审核
5. 根据 review 意见修改，保持对话简洁

## 问题与讨论

- Bug / 功能建议：提 GitHub Issue（使用模板）
- 设计相关问题：先阅读 `skills.md` 了解 UI 风格约束
- 不确定的改动：先开 issue 讨论方案，再动手实现

## 行为准则

- 友善沟通，就事论事
- 尊重既有设计决策，如需推翻请附理由和对比
- 不提交与任务无关的大规模重构
