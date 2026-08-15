---
name: game-launcher-ui
description: Use when designing or implementing the game launcher's UI. Applies modern minimal dark-theme visual specs for sidebar, game grid, game detail page, search bar, and interaction feedback. Use with Tailwind CSS + lucide-react.
---

# Game Launcher Modern UI Style

## 适用场景
当用户要求设计或实现游戏启动器的界面时，自动应用以下视觉规范。

## 设计理念
- **现代极简**：参考 Sofast / Raycast / Playnite Modern UI。
- **响应式**：窗口拉伸时，游戏网格自动调整列数。
- **暗色模式优先**（提供浅色模式切换）。

## 颜色方案（暗色主题默认）
- 背景色：`#0F0F0F`（主背景），`#1A1A1A`（侧边栏/卡片）
- 前景文字：`#E5E5E5`（主要文字），`#A1A1A1`（次要文字）
- 强调色：`#3B82F6`（蓝色）用于按钮、选中状态
- 成功/警告/错误：`#10B981`, `#F59E0B`, `#EF4444`

注意：`#0F0F0F` 对应 Tailwind CSS 没有精确匹配，定义 `colors.main` 到 Tailwind 配置为 `bg-main`。

## 组件样式
### 侧边栏 (Sidebar)
- 宽度 240px，背景 `#0A0A0A`。
- 菜单项：圆角 8px，hover 高亮，选中时带左侧蓝色条。
- 字体：Inter / SF Pro，14px。

### 游戏网格 (Game Grid)
- 卡片尺寸：180px 宽 × 200px 高（可调）。
- 卡片圆角 12px，背景 `#1A1A1A`，边框 `#2A2A2A`。
- hover 时卡片轻微上浮（`translateY(-4px)`）+ 阴影。
- 图标：居中显示，最大 100px 宽高，下方显示游戏名称（最多两行）。
- 最近游玩时间显示在卡片右下角（小字）。

### 游戏详情页 (Game Detail)
- 封面大图（若可从 Steam 获取）铺满顶部区域，叠加半透明渐变。
- 标题 + 游玩时长 + 标签行。
- 四个主要按钮（启动、打开文件夹、编辑、设置状态）水平排列。
- 简介区域：最大高度 200px，滚动。
- 截图缩略图（若有）：横向滚动列表。

### 搜索与排序栏
- 顶部栏高度 56px，背景 `#0F0F0F`，右侧放置搜索输入框和下拉排序。
- 搜索框圆角 20px，背景 `#2A2A2A`，带图标。
- 排序下拉按钮样式统一。

## 交互反馈
- 按钮 hover 时透明度或缩放微动。
- 加载过程显示骨架屏或旋转加载器（颜色为强调色）。
- 游戏启动失败弹出 toast 通知（右下角，3秒消失）。

## 技术实现约束
- 使用 Tailwind CSS 实现所有样式，避免自定义 CSS（除非必要）。
- 图标库使用 `lucide-react`（若 React）或相应框架的等价图标集。
- 支持窗口最小尺寸 800×600，布局在此尺寸下不变形。
