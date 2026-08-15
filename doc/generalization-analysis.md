# PotatoVN 通用化改造分析

> 目标：将 PotatoVN 从 Visual Novel 专用启动器改造为支持所有游戏的通用启动器

---

## 一、当前架构中 VN 专用 vs 通用 的判定

### 1.1 通用/可复用部分（约 70%）

| 模块 | 说明 |
|------|------|
| **游戏库管理** | `GalgameSourceBase` 抽象类、`LocalFolderSourceService` 文件夹监控、`SteamSourceService` Steam 库读取、`VirtualSourceService` 云端游戏 —— 这些对任何游戏类型都适用 |
| **游玩时间追踪** | `RecordPlayTimeTask` 监控进程并累加时长，`PlayedTime` / `PlayCount` / `TotalPlayTime` 属性完全是通用的 |
| **分类/标签/过滤** | `Category`、`TagFilter`、`SourceFilter`、`CategoryFilter` 的概念是通用的 |
| **播放状态** | `PlayType` 枚举（想玩 / 在玩 / 玩过 / 搁置 / 抛弃）适用于任何游戏 |
| **插件系统** | `IPlugin` + `IPotatoVnApi` 提供了良好的扩展机制 |
| **MVVM + XAML 框架** | WinUI 3 的页面、导航、设置等基础框架直接可用 |
| **封面图/头部图下载** | 图片获取和本地缓存逻辑通用 |

### 1.2 VN 专用/需改造部分（约 30%）

| 模块 | 具体问题 | 涉及文件 |
|------|----------|----------|
| **`Galgame` 类型名** | 渗透到每一个接口、服务、ViewModel、View、Filter 的签名中 | ~50+ 文件 |
| **`GalgameUid`** | 硬编码绑定了 Bangumi/VNDB/Ymgal/Steam 5 个 VN 数据库的 ID | `Galgame.cs:31-47` |
| **`RssType` 枚举 + `Ids[8]` 定长数组** | 不可扩展的固定数据源索引，`PhraserNumber = 8` 是硬上限 | `RssType.cs`, `Galgame.cs:99` |
| **`MixedPhraser` + 各独立 Phraser** | `BgmPhraser`、`VndbPhraser`、`YmgalPhraser`、`CngalPhraser` 全是 VN 专属信息源 | `Helpers/Phrase/` 目录 |
| **`Staff`(制作人员) 服务** | VN 特有的声优/画师/编剧追踪 | `IStaffService`, `Staff.cs` |
| **`IPvnService`** | PotatoVN 私有云同步服务 | `Services/AccountServices/PvnService.cs` |
| **VN 专属属性** | `RunInLocaleEmulator`、`EnableMagpie`、`MuteInBackground`、`ExpectedPlayTime`(来自 VNDB) | `Galgame.cs:61-86` |
| **`IGalgamePage` 插件面板** | 名称带 `Galgame`，但概念上换名即可 | `Contracts/PluginUi/IGalgamePage.cs` |

---

## 二、改造策略：渐进式三步走

### 第一步：抽象出 `IGame` 接口

这是最关键的一步。当前 `Galgame` 是一个 428 行的 `partial class`，直接作为数据模型被所有层引用。

```
Galgame (当前, 428行)
  │
  ├── 提取接口 IGame ── 含有通用属性:
  │     Name, Description, Developer, Rating, Tags
  │     PlayType, PlayedTime, TotalPlayTime, PlayCount, LastPlayTime
  │     ImagePath, HeaderImagePath, ReleaseDate
  │     Uuid, Sources, IsLocalGame
  │     ExePath, ExeArguments, SavePosition (游戏启动相关)
  │
  └── Galgame 保留 VN 专属部分:
        Ids[] (VN数据库ID数组)
        GalgameUid, RssType
        Staff, Characters (VN声优/画师)
        ExpectedPlayTime, RunInLocaleEmulator, EnableMagpie
```

**修改范围估算：**

| 改动项 | 涉及文件数 | 工作量 |
|--------|-----------|--------|
| `IGame` 接口新增 | 1 | 低 |
| `Galgame` 重构为实现 `IGame` | 1 | 低 |
| `FilterBase.Apply(Galgame)` → `FilterBase.Apply(IGame)` | ~15 | 低 |
| `IGalgameCollectionService` → 新增 `IGameCollectionService` | 2 | 中 |
| ViewModel 中 `ObservableCollection<Galgame>` → `ObservableCollection<IGame>` | ~10 | 中 |

### 第二步：将信息源系统从枚举改为注册表模式

当前最大的架构硬伤是 `RssType` 枚举 + `Ids[8]` 定长数组。

```csharp
// 旧方案：定长数组 + 枚举索引
public enum RssType { Vndb = 0, Bangumi = 1, ... }
public string?[] Ids = new string[8];

// 新方案：字符串键的字典 + 动态注册
public Dictionary<string, string> ExternalIds { get; } = [];
// 注册：scraperRegistry.Register("igdb", new IgdbScraper());
// 存储：game.ExternalIds["igdb"] = "12345";
```

**具体改动：**

1. `PhraserList` 从 `Dictionary<int, IGalInfoPhraser>` 改为 `Dictionary<string, IGameInfoScraper>`
   - 文件：`GalgameCollectionService.cs:45-76`
   - 影响：约 30 处 `PhraserList[(int)RssType.Xxx]` 调用改为 `PhraserList["xxx"]`

2. `IGalInfoPhraser.GetPhraseType()` 返回 `string` 而非 `RssType`
   - 文件：`IGalInfoPhraser.cs:21`
   - 接口改名：`IGalInfoPhraser` → `IGameInfoScraper`
   - 方法改名：`GetGalgameInfo(Galgame)` → `GetGameInfo(IGame)`

3. VN 数据库作为默认注册的 scraper 保持不变
   - Bangumi/VNDB 等仍然可以注册，只是注册方式从 enum hardcode 变为字符串 key

4. 新的通用游戏数据库 scraper 可通过插件或内置注册

### 第三步：服务层分离

将 VN 专用的子服务从 `GalgameCollectionService` 中抽离：

```
当前 GalgameCollectionService (1200+ 行, 混合了通用逻辑和VN逻辑)
  │
  ├── 保留: IGameCollectionService
  │     AddGameAsync, RemoveGame, GetGameByXxx
  │     游戏库 CRUD、重名检测、meta.json 读写
  │
  ├── 抽离为插件: IGalInfoPhraser (VN信息抓取)
  │     MixedPhraser, BgmPhraser, VndbPhraser, YmgalPhraser...
  │     作为实现 IGameInfoScraper 的插件加载
  │
  └── 抽离: IPvnService → 单独的服务
        云端同步作为可选功能
```

---

## 三、具体文件修改清单

### 3.1 核心模型层 (`GalgameManager.WinApp.Base/`)

| 文件 | 改动 | 工作量 |
|------|------|--------|
| `Models/Galgame.cs:18` | `class Galgame : IGame`，提取接口 | 中 |
| `Models/Sources/GalgameSourceBase.cs:24` | `List<GalgameAndPath>` 中的 `Galgame` 可保留 | 低 |
| `Enums/RssType.cs` | 废弃或改为 `const string` 常量类 | 低 |
| `Contracts/Phrase/IGalInfoPhraser.cs` | 重命名为 `IGameInfoScraper`，参数改为 `IGame` | 中 |
| `Contracts/Phrase/IGalCharacterPhraser.cs` | 可选保留或改为插件 | 低 |
| `Contracts/Phrase/IGalStaffParser.cs` | 改为 VN 插件 | 低 |
| `Contracts/Phrase/IGalCoversParser.cs` | 重命名 `ICoverScraper`，参数改 `IGame` | 低 |
| `Contracts/Phrase/IGalHeadersParser.cs` | 重命名 `IHeaderScraper`，参数改 `IGame` | 低 |
| `Models/Filters/FilterBase.cs:9` | `Apply(Galgame)` → `Apply(IGame)` | 低 |
| `Helpers/RssTypeHelper.cs` | 重构为 `ScraperRegistry` | 中 |

### 3.2 服务层 (`GalgameManager/Services/`)

| 文件 | 改动 | 工作量 |
|------|------|--------|
| `GalgameCollectionService/GalgameCollectionService.cs:28` | `ObservableCollection<Galgame>` 保留下层类型，对外暴露 `IReadOnlyList<IGame>` | 中 |
| `GalgameCollectionService/GalgameCollectionService_AddGame.cs` | `AddGameAsync` 返回类型从 `Galgame` 改为 `IGame` | 中 |
| `Contracts/Services/IGalgameCollectionService.cs` | 新增 `IGameCollectionService`，旧接口标记 `[Obsolete]` | 高 |
| `Contracts/Services/IGalgameSourceCollectionService.cs` | 同理新增 `IGameSourceCollectionService` | 中 |
| `Contracts/Services/IFilterService.cs:24` | `ApplyFilters(Galgame)` → `ApplyFilters(IGame)` | 低 |
| `Contracts/Services/ICategoryService.cs:90` | `GetDeveloperCategory(Galgame)` → `GetDeveloperCategory(IGame)` | 中 |
| `Services/SourceService/LocalFolderSourceService.cs` | 基本不变，`meta.json` 读写逻辑保持 | 低 |
| `Services/SourceService/SteamSourceService.cs` | 基本不变 | 低 |

### 3.3 ViewModel 层 (`GalgameManager/ViewModels/`)

| 文件 | 改动 | 工作量 |
|------|------|--------|
| `HomeViewModel.cs:79` | `AdvancedCollectionView` 的源类型从 `List<Galgame>` 改为 `List<IGame>` | 中 |
| `GalgameViewModel.cs:44` | `[ObservableProperty] Galgame? _item` 改为 `IGame?`，VN 特有属性通过 `is Galgame g` 模式判断 | 高 |
| `PlayedTimeViewModel.cs:18` | `Galgame Game` → `IGame Game` | 低 |
| `SettingsViewModel.cs` | 基本不变，设置项偏通用 | 低 |

### 3.4 View 层 (`GalgameManager/Views/`)

| 文件 | 改动 | 工作量 |
|------|------|--------|
| `HomePage.xaml` | 基本不变，绑定通过 ViewModel 间接适配 | 低 |
| `GalgamePage.xaml` | `x:Uid` 前缀从 `GalgamePage_` 改为通用命名 | 中 |
| `PlayedTimePage.xaml` | 基本不变 | 低 |
| 各 Dialog XAML | `x:Uid` 前缀和导航参数类型适配 | 低 |

---

## 四、新增通用游戏数据库的建议

如果要支持所有游戏，推荐接入以下信息源（仿照现有 Phraser 模式实现 `IGameInfoScraper`）：

| 数据库 | 覆盖范围 | API 特点 |
|--------|----------|----------|
| **IGDB** (igdb.com) | 全平台全游戏 | Twitch 旗下，REST API，需注册，免费额度充足 |
| **SteamGridDB** | Steam 游戏封面/横幅/图标 | 专门提供游戏美术资源，REST API |
| **RAWG** (rawg.io) | 全平台 50万+ 游戏 | REST API，免费 tier |
| **Steam Store API** | Steam 游戏（已有 `SteamParser.cs`，可直接复用） | 已有实现，Refit |
| **GiantBomb** | 全平台 | Wiki 式游戏数据库，REST API |

每个 scraper 可参考现有 `SteamParser.cs` (171行) 的模式实现：

1. 实现 `IGameInfoScraper` 接口
2. `GetGameInfo(IGame)` 中使用游戏名搜索 API
3. 使用 Jaro-Winkler 相似度做模糊匹配
4. 获取详情后填充名字/简介/封面/评分/标签/开发商
5. 写入 `game.ExternalIds["source_key"]`
6. 可选实现 `ICoverScraper` / `IHeaderScraper` 获取美术资源

### 新 scraper 的注册方式

```csharp
// 方式一：插件注册（推荐，无需改核心代码）
public class IgdbPlugin : IPlugin, IParserProvider
{
    public IGameInfoScraper GetScraper() => new IgdbScraper();
    public string ScraperName => "IGDB";
}

// 方式二：内置注册（在 GalgameCollectionService 构造函数中）
scraperRegistry.Register("igdb", new IgdbScraper());
scraperRegistry.Register("rawg", new RawgScraper());
```

---

## 五、改造优先级

| 顺序 | 步骤 | 理由 |
|------|------|------|
| 1 | **提取 `IGame` 接口** + 修改 `FilterBase`/`ICategoryService` 签名 | 基座改造，完成后后续改造不会破坏现有功能 |
| 2 | **Phraser 系统注册表化**（enum → string key）+ 新增 IGDB/RAWG scraper | 立即可用新数据源，插件生态系统扩展 |
| 3 | **`GalgameCollectionService` 拆分为 `IGameCollectionService` + VN 插件** | 解耦 VN 和通用逻辑 |
| 4 | **ViewModel 层适配 `IGame`** | 最后改 UI 层，不影响数据层 |

> **注意**：项目现有的插件系统（`IPlugin` + `IParserProvider` + `ISourceProvider`）已经设计得很好，新增的通用 scraper 完全可以通过插件方式加载，不需要改核心代码。最大的工作量在于第一步的接口提取和类型替换——好在 C# 的 IDE 重构工具（Rider/VS）可以自动化大部分重命名。

---

## 六、关键接口改名对照表

| 旧名称 | 新名称 | 说明 |
|--------|--------|------|
| `Galgame` | 实现 `IGame` | 保留原类，新增接口 |
| `IGalInfoPhraser` | `IGameInfoScraper` | 游戏信息抓取器 |
| `IGalCharacterPhraser` | `ICharacterScraper` | 角色信息抓取器 |
| `IGalStaffParser` | `IStaffScraper` | 制作人员抓取器 |
| `IGalCoversParser` | `ICoverScraper` | 封面图抓取器 |
| `IGalHeadersParser` | `IHeaderScraper` | 头部横幅抓取器 |
| `IGalStatusSync` | `IPlayStatusSync` | 游玩状态同步 |
| `IGalgameCollectionService` | `IGameCollectionService` | 游戏集合服务 |
| `IGalgameSourceCollectionService` | `IGameSourceCollectionService` | 游戏源集合服务 |
| `GalgameCollectionService` | `GameCollectionService` | 游戏集合服务实现 |
| `IGalgameSourceService` | `IGameSourceService` | 游戏源服务 |
| `RssType` | `ScraperKey`（废弃枚举，改用字符串） | 信息源标识 |
