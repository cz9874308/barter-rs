# Rust 中文注释规范

## 📋 概述

本文档基于实际项目代码（参考 `mini-rs-v0.0.1` 项目）总结的 Rust 中文注释规范，旨在提供清晰、统一、易读的代码文档标准。

## 🎯 注释类型

Rust 支持三种类型的注释：

| 注释类型              | 语法  | 用途                           | 是否生成文档 |
| --------------------- | ----- | ------------------------------ | ------------ |
| **模块级文档注释**    | `//!` | 描述整个模块的功能             | ✅ 是        |
| **公共 API 文档注释** | `///` | 描述公共函数、结构体、trait 等 | ✅ 是        |
| **行内注释**          | `//`  | 解释代码逻辑                   | ❌ 否        |

---

## 📝 模块级文档注释 (`//!`)

### 使用场景

用于描述整个模块（`mod`）的功能、设计理念和使用方式。

### 基本格式

```rust
//! 模块简要描述
//!
//! 模块的详细说明，可以包含多行。
//!
//! # 核心概念
//!
//! - **概念1**: 说明
//! - **概念2**: 说明
//!
//! # 使用方式
//!
//! 1. 步骤一
//! 2. 步骤二
//!
//! # 注意事项
//!
//! 重要提示或警告信息。
```

### 示例

```rust
//! 服务依赖注入模块
//!
//! 本模块实现了编译时依赖注入功能，允许服务自动注入依赖的组件和配置。
//!
//! # 核心概念
//!
//! - **Service**: 特殊的组件，支持通过字段属性自动注入依赖
//! - **ServiceRegistrar**: 服务注册器，负责将服务安装到应用中
//! - **自动注入**: 在应用构建时自动发现并安装所有服务
//!
//! # 使用方式
//!
//! 1. 定义服务结构体，使用 `#[derive(Service)]` 宏
//! 2. 使用 `#[inject(component)]` 或 `#[inject(config)]` 标记需要注入的字段
//! 3. 框架会在应用构建时自动发现并安装所有服务
//!
//! # 依赖解析
//!
//! 服务安装时会自动解析依赖关系，确保依赖的服务先于被依赖的服务安装。
```

### 推荐章节结构

-   **概述**：模块的核心功能和目的
-   **核心概念**：关键术语和概念解释
-   **使用方式**：基本使用步骤
-   **工作原理**：内部实现机制（如需要）
-   **注意事项**：重要提示或限制

---

## 📚 公共 API 文档注释 (`///`)

### 使用场景

用于描述公共函数、结构体、枚举、trait、常量等公共 API。

### 基本格式

````rust
/// 功能简要描述
///
/// 详细说明，可以包含多行。
///
/// # 实现要求
///
/// 实现此 trait 或使用此函数的要求。
///
/// # 参数
///
/// - `param1`: 参数说明
/// - `param2`: 参数说明
///
/// # 返回值
///
/// 返回值说明。
///
/// # 错误处理
///
/// 可能的错误情况。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 示例代码
/// ```
///
/// # 工作原理
///
/// 内部实现机制说明（如需要）。
````

### 示例：函数注释

````rust
/// 自动注入所有服务
///
/// 查找所有已注册的 `ServiceRegistrar` 并将它们安装到应用中。
///
/// # 工作原理
///
/// 1. 收集所有通过 `inventory` 注册的服务注册器
/// 2. 循环安装服务，直到所有服务都安装成功：
///    - 尝试安装每个待安装的服务
///    - 如果成功，从待安装列表移除
///    - 如果失败（可能是依赖未满足），保留在待安装列表
/// 3. 如果一轮中没有进展，说明存在循环依赖或缺少依赖，返回错误
///
/// # 参数
///
/// - `app`: 应用构建器
///
/// # 返回值
///
/// 如果所有服务都成功安装，返回 `Ok(())`；否则返回错误
///
/// # 依赖解析
///
/// 使用拓扑排序算法解析服务依赖：
/// - 如果服务的依赖都已安装，则可以安装该服务
/// - 如果依赖未满足，等待下一轮
/// - 如果一轮中没有进展，说明存在循环依赖或缺少依赖
///
/// # 示例
///
/// ```rust,ignore
/// let mut app = App::new();
/// service::auto_inject_service(&mut app)?;
/// ```
pub fn auto_inject_service(app: &mut AppBuilder) -> Result<()> {
    // 实现代码
}
````

### 示例：Trait 注释

````rust
/// Service trait：支持依赖注入的特殊组件
///
/// Service 是一种特殊的组件，可以通过字段属性自动注入依赖的组件和配置。
///
/// # 实现要求
///
/// - 必须实现 `Clone + Sized + 'static`
/// - 必须实现 `build()` 方法，用于从注册表构建服务实例
///
/// # 使用示例
///
/// ```rust,ignore
/// use mini::plugin::service::Service;
/// use mini_sqlx::ConnectPool;
///
/// #[derive(Clone, Service)]
/// struct UserService {
///     // 注入组件
///     #[inject(component)]
///     db: ConnectPool,
///
///     // 注入配置
///     #[inject(config)]
///     config: UserConfig,
/// }
/// ```
///
/// # 工作原理
///
/// 1. `#[derive(Service)]` 宏会生成 `ServiceRegistrar` 实现
/// 2. 服务在编译时通过 `inventory` 自动注册
/// 3. 应用构建时，`auto_inject_service()` 会遍历所有注册的服务
/// 4. 按照依赖顺序安装服务，确保依赖的服务先安装
pub trait Service: Clone + Sized + 'static {
    // trait 方法
}
````

### 示例：方法注释

```rust
/// 从注册表构建服务组件
///
/// 此方法由 `#[derive(Service)]` 宏自动生成，会：
/// 1. 从组件注册表获取标记为 `#[inject(component)]` 的字段
/// 2. 从配置注册表获取标记为 `#[inject(config)]` 的字段
/// 3. 构建并返回服务实例
///
/// # 类型参数
///
/// - `R`: 注册表类型，必须同时实现 `ComponentRegistry` 和 `ConfigRegistry`
///
/// # 参数
///
/// - `registry`: 组件和配置注册表
///
/// # 返回值
///
/// 如果成功，返回构建的服务实例；否则返回错误
fn build<R>(registry: &R) -> Result<Self>
where
    R: ComponentRegistry + ConfigRegistry;
```

### 推荐章节结构

根据 API 的复杂度，可以包含以下章节：

-   **概述**：功能简要描述（必需）
-   **实现要求**：对于 trait，说明实现要求
-   **参数**：函数参数说明
-   **类型参数**：泛型参数说明
-   **返回值**：返回值说明
-   **错误处理**：可能的错误情况
-   **使用示例**：代码示例（强烈推荐）
-   **工作原理**：内部实现机制（如需要）
-   **注意事项**：重要提示或限制

---

### 复杂函数的人性化注释指南

对于复杂的函数，需要编写更加人性化、通俗易懂的文档注释，帮助读者快速理解函数的目的、工作原理和使用方式。

#### 编写原则

1. **用通俗语言解释**：避免过于技术化的术语，用日常语言解释概念
2. **提供背景信息**：说明函数解决什么问题，为什么需要这个函数
3. **分步骤说明**：将复杂过程分解为清晰的步骤
4. **使用类比**：用熟悉的概念类比复杂的概念
5. **提供实际场景**：说明在什么情况下使用这个函数
6. **解释关键决策**：说明为什么这样实现，有什么考虑

#### 对比示例：不好的注释 vs 好的注释

##### ❌ 不好的注释（过于技术化，难以理解）

```rust
/// 自动注入所有服务
///
/// 遍历 inventory 收集的 ServiceRegistrar，通过拓扑排序解析依赖，
/// 循环调用 install_service 直到所有服务安装完成或检测到循环依赖。
///
/// # 参数
///
/// - `app`: AppBuilder 实例
///
/// # 返回值
///
/// Result<()>
pub fn auto_inject_service(app: &mut AppBuilder) -> Result<()> {
    // 实现
}
```

**问题**：

-   直接使用技术术语（inventory、拓扑排序），没有解释
-   没有说明为什么需要这个函数
-   没有解释"循环依赖"是什么，为什么会出现
-   缺少使用场景说明

##### ✅ 好的注释（人性化、通俗易懂）

````rust
/// 自动注入所有服务到应用中
///
/// 这个函数的作用就像"自动装配工"，它会自动找到所有需要安装的服务，
/// 并按照正确的顺序把它们安装到应用中。
///
/// ## 为什么需要这个函数？
///
/// 在应用中，服务之间可能存在依赖关系。比如：
/// - 用户服务需要数据库连接
/// - 订单服务需要用户服务和数据库连接
///
/// 如果先安装订单服务，再安装用户服务，就会出错，因为订单服务依赖的用户服务还不存在。
/// 这个函数会自动解决这个问题，确保依赖的服务总是先于被依赖的服务安装。
///
/// ## 工作原理（简单理解）
///
/// 想象一下整理书架的过程：
///
/// 1. **收集所有书**：找到所有需要安装的服务（就像收集所有要放上书架的书）
/// 2. **尝试摆放**：尝试安装每个服务
///    - 如果服务的依赖都已经安装好了，就安装这个服务（就像书的前置书籍都已经在书架上了）
///    - 如果依赖还没安装，就暂时跳过，等下一轮再试（就像前置书籍还没到，先放一边）
/// 3. **重复直到完成**：重复步骤 2，直到所有服务都安装成功
/// 4. **检测问题**：如果一轮下来没有任何进展，说明存在循环依赖或缺少依赖，返回错误
///
/// ## 实际使用场景
///
/// 通常在应用启动时调用，一次性安装所有服务：
///
/// ```rust,ignore
/// let mut app = App::new();
///
/// // 配置数据库、日志等基础组件
/// app.add_component(database);
/// app.add_component(logger);
///
/// // 自动安装所有服务（用户服务、订单服务等）
/// service::auto_inject_service(&mut app)?;
///
/// // 现在所有服务都可以使用了
/// app.run()?;
/// ```
///
/// ## 可能遇到的问题
///
/// ### 循环依赖
///
/// 如果服务 A 依赖服务 B，服务 B 又依赖服务 A，就会形成循环依赖。
/// 这种情况下，函数会返回错误，提示你检查服务的依赖关系。
///
/// ### 缺少依赖
///
/// 如果服务依赖的组件或配置还没有添加到应用中，函数也会返回错误。
/// 确保在调用此函数之前，所有必需的组件都已经添加到应用中。
///
/// # 参数
///
/// - `app`: 应用构建器，用于安装服务
///
/// # 返回值
///
/// - `Ok(())`: 所有服务都成功安装
/// - `Err(...)`: 安装失败，可能是循环依赖或缺少依赖
///
/// # 示例
///
/// ```rust,ignore
/// let mut app = App::new();
///
/// // 先添加基础组件
/// app.add_component(create_database_pool()?);
///
/// // 然后自动安装所有服务
/// service::auto_inject_service(&mut app)?;
///
/// println!("所有服务安装成功！");
/// ```
pub fn auto_inject_service(app: &mut AppBuilder) -> Result<()> {
    // 实现
}
````

**优点**：

-   用"自动装配工"类比，形象易懂
-   解释了为什么需要这个函数（依赖关系问题）
-   用"整理书架"类比工作原理，容易理解
-   提供了实际使用场景
-   说明了可能遇到的问题和解决方案

#### 复杂函数注释模板

````rust
/// [函数功能简要描述]
///
/// [用一句话或一个类比说明这个函数是做什么的，让读者快速理解]
///
/// ## 为什么需要这个函数？
///
/// [说明这个函数解决什么问题，为什么需要它]
///
/// ## 工作原理（简单理解）
///
/// [用通俗语言或类比解释工作原理，可以分步骤说明]
///
/// 1. [步骤一：用日常语言描述]
/// 2. [步骤二：用日常语言描述]
/// 3. [步骤三：用日常语言描述]
///
/// ## 实际使用场景
///
/// [说明在什么情况下使用这个函数，提供实际场景]
///
/// ```rust,ignore
/// // 使用示例
/// ```
///
/// ## 可能遇到的问题
///
/// ### [问题一]
///
/// [解释问题是什么，如何避免或解决]
///
/// ### [问题二]
///
/// [解释问题是什么，如何避免或解决]
///
/// # 参数
///
/// - `param1`: [参数说明，用通俗语言]
///
/// # 返回值
///
/// [返回值说明，用通俗语言]
///
/// # 示例
///
/// ```rust,ignore
/// // 完整的使用示例
/// ```
pub fn complex_function(param1: Type1) -> Result<Type2> {
    // 实现
}
````

#### 更多人性化注释示例

##### 示例 1：依赖解析函数

````rust
/// 解析服务依赖关系，确定安装顺序
///
/// 这个函数就像一个"任务调度器"，它会分析所有服务之间的依赖关系，
/// 然后告诉你应该按照什么顺序安装这些服务。
///
/// ## 为什么需要这个函数？
///
/// 想象你要做一顿饭：
/// - 做菜需要先洗菜
/// - 炒菜需要先切菜
/// - 切菜需要先洗菜
///
/// 如果你不按顺序来，比如先炒菜再洗菜，就会出问题。
/// 服务安装也是一样，必须按照依赖顺序来。
///
/// ## 工作原理
///
/// 这个函数使用"拓扑排序"算法（就像整理任务清单一样）：
///
/// 1. **建立依赖图**：画出所有服务之间的依赖关系
///    - 用户服务 → 依赖 → 数据库连接
///    - 订单服务 → 依赖 → 用户服务、数据库连接
///
/// 2. **找到起点**：找出不依赖任何其他服务的服务（就像找到可以立即开始的任务）
///
/// 3. **逐步扩展**：安装这些服务后，再安装依赖它们的服务
///
/// 4. **检测循环**：如果发现循环依赖（A 依赖 B，B 又依赖 A），返回错误
///
/// ## 实际使用场景
///
/// 通常在应用启动时，在安装服务之前调用，确保安装顺序正确：
///
/// ```rust,ignore
/// // 解析依赖关系
/// let install_order = resolve_dependencies(services)?;
///
/// // 按照顺序安装
/// for service in install_order {
///     service.install(&mut app)?;
/// }
/// ```
///
/// # 参数
///
/// - `services`: 所有需要安装的服务列表
///
/// # 返回值
///
/// - `Ok(Vec<Service>)`: 按安装顺序排列的服务列表
/// - `Err(...)`: 如果存在循环依赖，返回错误
pub fn resolve_dependencies(services: &[Service]) -> Result<Vec<Service>> {
    // 实现
}
````

##### 示例 2：缓存管理函数

````rust
/// 智能缓存管理器：自动决定何时更新缓存
///
/// 这个函数就像一个"智能管家"，它会自动判断缓存是否过期，
/// 如果过期了就更新，没过期就直接使用，让你不用每次都手动检查。
///
/// ## 为什么需要这个函数？
///
/// 假设你有一个用户信息缓存：
/// - 如果每次都去数据库查询，会很慢
/// - 如果一直用缓存，数据可能过期
///
/// 这个函数帮你自动处理这个平衡：需要时更新，不需要时用缓存。
///
/// ## 工作原理
///
/// 就像检查冰箱里的食物是否过期：
///
/// 1. **检查缓存是否存在**：看看冰箱里有没有这个食物
/// 2. **检查是否过期**：看看食物的保质期过了没有
///    - 如果没过期：直接使用（返回缓存的数据）
///    - 如果过期了：重新获取（去超市买新的，更新缓存）
/// 3. **返回结果**：给你最新的数据
///
/// ## 实际使用场景
///
/// 适用于需要定期更新的数据，比如用户信息、配置信息等：
///
/// ```rust,ignore
/// // 获取用户信息（自动处理缓存）
/// let user = get_cached_user(user_id)?;
///
/// // 如果缓存过期，会自动从数据库获取最新数据
/// // 如果缓存有效，直接返回缓存数据，速度很快
/// ```
///
/// ## 缓存策略
///
/// - **时间过期**：缓存超过指定时间后自动过期
/// - **版本过期**：如果数据版本号变化，缓存自动失效
/// - **手动刷新**：可以手动调用刷新缓存
///
/// # 参数
///
/// - `key`: 缓存的键（就像食物的名称）
/// - `ttl`: 缓存有效期（秒），就像食物的保质期
///
/// # 返回值
///
/// - `Ok(T)`: 缓存的值（可能是旧的，也可能是刚更新的）
/// - `Err(...)`: 获取数据失败
pub fn get_cached_user(key: &str, ttl: u64) -> Result<User> {
    // 实现
}
````

#### 人性化注释检查清单

编写复杂函数注释时，检查以下内容：

-   [ ] **有通俗易懂的类比**：用熟悉的概念解释复杂概念
-   [ ] **解释了"为什么"**：说明函数解决什么问题
-   [ ] **分步骤说明**：将复杂过程分解为清晰步骤
-   [ ] **提供使用场景**：说明什么时候使用这个函数
-   [ ] **说明可能的问题**：提前告知可能遇到的问题
-   [ ] **避免过度技术化**：用日常语言，少用专业术语
-   [ ] **提供完整示例**：包含实际可运行的代码示例
-   [ ] **解释关键决策**：说明为什么这样实现

#### 常见技术术语的通俗解释

| 技术术语     | 通俗解释                                                         | 使用场景 |
| ------------ | ---------------------------------------------------------------- | -------- |
| **拓扑排序** | 就像整理任务清单，找出先做什么后做什么                           | 依赖解析 |
| **依赖注入** | 就像点外卖，你不需要知道怎么做，只需要说你要什么，系统会自动给你 | 服务管理 |
| **缓存**     | 就像把常用的东西放在手边，需要时直接拿，不用每次都去远处取       | 性能优化 |
| **异步**     | 就像同时做多件事，不用等一件事做完再做另一件                     | 并发处理 |
| **生命周期** | 就像物品的保质期，告诉系统这个东西能用多久                       | 内存管理 |
| **泛型**     | 就像万能模板，可以适用于不同类型                                 | 代码复用 |
| **Trait**    | 就像"能力"或"技能"，定义了能做什么                               | 接口定义 |

---

## 💬 行内注释 (`//`)

### 使用场景

用于解释代码逻辑、算法步骤、关键决策等。

### 基本格式

```rust
// 简洁说明代码意图
let result = calculate();

// 多行注释可以这样写：
// 第一行说明
// 第二行说明
```

### 示例

```rust
// 收集所有服务注册器
let registrars: Vec<&'static &dyn ServiceRegistrar> =
    inventory::iter::<&dyn ServiceRegistrar>().collect();

// 循环安装服务，直到所有服务都安装成功
while !pending.is_empty() {
    let mut next_pending = Vec::new();
    let mut progress_made = false;

    // 尝试安装每个待安装的服务
    for registrar in pending {
        match registrar.install_service(app) {
            Ok(()) => {
                // 安装成功
                installed += 1;
                progress_made = true;
            }
            Err(_) => {
                // 安装失败（可能是依赖未满足），等待下一轮
                next_pending.push(registrar);
            }
        }
    }

    // 如果一轮中没有进展，说明存在循环依赖或缺少依赖
    if !progress_made && !next_pending.is_empty() {
        // 返回第一个失败的服务安装错误，提供更详细的错误信息
        return first.install_service(app);
    }
}
```

### 注释原则

-   **简洁明了**：注释应该简洁，避免冗长
-   **解释"为什么"**：优先解释代码的意图和原因，而非"是什么"
-   **避免重复**：不要重复代码本身已经表达清楚的内容
-   **及时更新**：代码修改时，同步更新相关注释

---

## 📖 Markdown 格式支持

Rust 文档注释支持完整的 Markdown 语法。

### 常用 Markdown 元素

#### 标题

```rust
/// # 一级标题
/// ## 二级标题
/// ### 三级标题
```

#### 列表

```rust
/// - 无序列表项 1
/// - 无序列表项 2
///
/// 1. 有序列表项 1
/// 2. 有序列表项 2
```

#### 强调

```rust
/// 这是 **粗体** 文本
/// 这是 *斜体* 文本
/// 这是 `代码` 文本
```

#### 代码块

````rust
/// ```rust,ignore
/// // 示例代码
/// let x = 42;
/// ```
````

**代码块类型说明**：

-   `rust`：可执行的 Rust 代码（会被测试）
-   `rust,ignore`：Rust 代码示例，但不执行
-   `rust,no_run`：Rust 代码，编译但不运行
-   `text`：纯文本

#### 表格

```rust
/// | 列1 | 列2 | 列3 |
/// |-----|-----|-----|
/// | 值1 | 值2 | 值3 |
```

#### 链接

```rust
/// 查看 [Rust 官方文档](https://doc.rust-lang.org/)
/// 参考 [`Service`](struct.Service.html) trait
```

---

## ✅ 注释规范检查清单

### 模块级注释 (`//!`)

-   [ ] 包含模块的简要描述
-   [ ] 说明核心概念（如需要）
-   [ ] 提供使用方式或示例（如需要）
-   [ ] 说明注意事项或限制（如需要）

### 公共 API 注释 (`///`)

-   [ ] 包含功能简要描述
-   [ ] 说明参数（如有）
-   [ ] 说明返回值
-   [ ] 说明错误处理（如可能出错）
-   [ ] 提供使用示例（强烈推荐）
-   [ ] 说明实现要求（对于 trait）

### 行内注释 (`//`)

-   [ ] 解释复杂逻辑
-   [ ] 说明关键决策
-   [ ] 避免重复代码本身

### 通用要求

-   [ ] 使用中文编写注释
-   [ ] 使用 Markdown 格式
-   [ ] 保持注释与代码同步
-   [ ] 代码示例可以编译（如使用 `rust` 而非 `rust,ignore`）

---

## 🎨 注释风格示例

### 完整示例：模块 + Trait + 方法

````rust
//! 服务依赖注入模块
//!
//! 本模块实现了编译时依赖注入功能，允许服务自动注入依赖的组件和配置。

use crate::app::AppBuilder;
use crate::error::Result;

/// Service trait：支持依赖注入的特殊组件
///
/// Service 是一种特殊的组件，可以通过字段属性自动注入依赖的组件和配置。
///
/// # 实现要求
///
/// - 必须实现 `Clone + Sized + 'static`
/// - 必须实现 `build()` 方法，用于从注册表构建服务实例
///
/// # 使用示例
///
/// ```rust,ignore
/// #[derive(Clone, Service)]
/// struct UserService {
///     #[inject(component)]
///     db: ConnectPool,
/// }
/// ```
pub trait Service: Clone + Sized + 'static {
    /// 从注册表构建服务组件
    ///
    /// 此方法由 `#[derive(Service)]` 宏自动生成，会：
    /// 1. 从组件注册表获取标记为 `#[inject(component)]` 的字段
    /// 2. 从配置注册表获取标记为 `#[inject(config)]` 的字段
    /// 3. 构建并返回服务实例
    ///
    /// # 参数
    ///
    /// - `registry`: 组件和配置注册表
    ///
    /// # 返回值
    ///
    /// 如果成功，返回构建的服务实例；否则返回错误
    fn build<R>(registry: &R) -> Result<Self>
    where
        R: ComponentRegistry + ConfigRegistry;
}

/// 自动注入所有服务
///
/// 查找所有已注册的 `ServiceRegistrar` 并将它们安装到应用中。
///
/// # 参数
///
/// - `app`: 应用构建器
///
/// # 返回值
///
/// 如果所有服务都成功安装，返回 `Ok(())`；否则返回错误
///
/// # 示例
///
/// ```rust,ignore
/// let mut app = App::new();
/// service::auto_inject_service(&mut app)?;
/// ```
pub fn auto_inject_service(app: &mut AppBuilder) -> Result<()> {
    // 收集所有服务注册器
    let registrars: Vec<&'static &dyn ServiceRegistrar> =
        inventory::iter::<&dyn ServiceRegistrar>().collect();

    // 循环安装服务，直到所有服务都安装成功
    while !pending.is_empty() {
        // 实现逻辑
    }

    Ok(())
}
````

---

## 🏷️ 变量命名规范

### 概述

Rust 遵循严格的命名约定，不同类型的标识符使用不同的命名风格。遵循这些约定可以让代码更易读、更符合 Rust 社区习惯。

### 命名风格对照表

| 标识符类型       | 命名风格               | 示例                           | 说明                 |
| ---------------- | ---------------------- | ------------------------------ | -------------------- |
| **结构体**       | `PascalCase`           | `UserService`, `Middlewares`   | 首字母大写的驼峰命名 |
| **枚举**         | `PascalCase`           | `HttpMethod`, `ErrorKind`      | 首字母大写的驼峰命名 |
| **Trait**        | `PascalCase`           | `Service`, `ComponentRegistry` | 首字母大写的驼峰命名 |
| **类型别名**     | `PascalCase`           | `Result<T>`, `ConfigMap`       | 首字母大写的驼峰命名 |
| **函数**         | `snake_case`           | `auto_inject_service`, `build` | 小写下划线命名       |
| **变量**         | `snake_case`           | `user_name`, `app_builder`     | 小写下划线命名       |
| **结构体字段**   | `snake_case`           | `compression`, `limit_payload` | 小写下划线命名       |
| **模块**         | `snake_case`           | `service`, `plugin`            | 小写下划线命名       |
| **常量**         | `SCREAMING_SNAKE_CASE` | `MAX_SIZE`, `DEFAULT_TIMEOUT`  | 全大写下划线命名     |
| **静态变量**     | `SCREAMING_SNAKE_CASE` | `GLOBAL_CONFIG`, `APP_NAME`    | 全大写下划线命名     |
| **生命周期参数** | 小写字母               | `'a`, `'static`                | 单引号 + 小写字母    |
| **泛型参数**     | `PascalCase`           | `T`, `R`, `E`                  | 单字母或 PascalCase  |

---

### 结构体命名

#### 基本规则

-   使用 `PascalCase` 命名
-   名称应该是名词或名词短语
-   避免缩写，除非是广泛使用的（如 `HTTP`, `URL`, `ID`）

#### 示例

```rust
/// 中间件配置结构体
///
/// 用于配置应用的各种中间件组件。
#[derive(Debug, Clone, Deserialize)]
pub struct Middlewares {
    /// 启用压缩中间件
    pub compression: Option<EnableMiddleware>,

    /// 限制请求体大小中间件
    pub limit_payload: Option<LimitPayloadMiddleware>,

    /// 追踪日志中间件
    pub logger: Option<TraceLoggerMiddleware>,
}

/// 用户服务结构体
pub struct UserService {
    // 字段定义
}

/// HTTP 请求配置
pub struct HttpRequestConfig {
    // 字段定义
}
```

---

### 结构体字段命名

#### 基本规则

-   使用 `snake_case` 命名
-   名称应该清晰描述字段的用途
-   布尔类型字段可以使用 `is_`, `has_`, `should_` 等前缀

#### 示例

```rust
/// 中间件配置结构体
#[derive(Debug, Clone, Deserialize)]
pub struct Middlewares {
    /// 启用压缩中间件
    pub compression: Option<EnableMiddleware>,

    /// 限制请求体大小中间件
    pub limit_payload: Option<LimitPayloadMiddleware>,

    /// 追踪日志中间件
    pub logger: Option<TraceLoggerMiddleware>,

    /// 捕获 panic 中间件
    pub catch_panic: Option<EnableMiddleware>,

    /// 请求超时中间件
    pub timeout_request: Option<TimeoutRequestMiddleware>,

    /// CORS 中间件
    pub cors: Option<CorsMiddleware>,

    /// 静态资源服务中间件
    ///
    /// 注意：序列化时字段名为 `static`，而非 `static_assets`
    #[serde(rename = "static")]
    pub static_assets: Option<StaticAssetsMiddleware>,
}

/// 用户信息结构体
pub struct User {
    /// 用户 ID
    pub user_id: u64,

    /// 用户名
    pub user_name: String,

    /// 邮箱地址
    pub email: String,

    /// 是否已激活
    pub is_active: bool,

    /// 是否已验证邮箱
    pub has_verified_email: bool,
}
```

#### 序列化时的命名处理

当 Rust 代码中的字段名与外部格式（如 JSON、YAML）的字段名不同时，使用 `serde` 属性：

```rust
/// 配置结构体
#[derive(Deserialize)]
pub struct Config {
    /// 静态资源配置
    ///
    /// 在 JSON 中字段名为 `static`，但 Rust 中 `static` 是关键字，
    /// 所以使用 `static_assets` 作为字段名，并通过 `serde(rename)` 映射
    #[serde(rename = "static")]
    pub static_assets: Option<StaticAssetsConfig>,

    /// 请求体大小限制
    ///
    /// 在 JSON 中使用 kebab-case，Rust 中使用 snake_case
    #[serde(rename = "limit-payload")]
    pub limit_payload: Option<u64>,
}
```

---

### 函数和变量命名

#### 基本规则

-   使用 `snake_case` 命名
-   函数名应该是动词或动词短语
-   变量名应该是名词或名词短语
-   布尔变量使用 `is_`, `has_`, `should_`, `can_` 等前缀

#### 函数命名示例

```rust
/// 自动注入所有服务
pub fn auto_inject_service(app: &mut AppBuilder) -> Result<()> {
    // 实现
}

/// 从注册表构建服务组件
pub fn build_service<R>(registry: &R) -> Result<Service> {
    // 实现
}

/// 检查用户是否存在
pub fn check_user_exists(user_id: u64) -> bool {
    // 实现
}

/// 获取用户信息
pub fn get_user_info(user_id: u64) -> Result<User> {
    // 实现
}
```

#### 变量命名示例

```rust
/// 自动注入所有服务
pub fn auto_inject_service(app: &mut AppBuilder) -> Result<()> {
    // 收集所有服务注册器
    let registrars: Vec<&'static &dyn ServiceRegistrar> =
        inventory::iter::<&dyn ServiceRegistrar>().collect();

    let total = registrars.len();
    let mut pending: Vec<&'static &dyn ServiceRegistrar> = registrars;
    let mut installed = 0;
    let mut progress_made = false;

    // 循环安装服务
    while !pending.is_empty() {
        let mut next_pending = Vec::new();

        // 尝试安装每个待安装的服务
        for registrar in pending {
            match registrar.install_service(app) {
                Ok(()) => {
                    installed += 1;
                    progress_made = true;
                }
                Err(_) => {
                    next_pending.push(registrar);
                }
            }
        }

        pending = next_pending;
    }

    Ok(())
}
```

#### 布尔变量命名

```rust
// 使用 is_ 前缀
let is_active = true;
let is_verified = false;
let is_empty = vec.is_empty();

// 使用 has_ 前缀
let has_permission = check_permission();
let has_children = node.has_children();

// 使用 should_ 前缀
let should_retry = error.is_retryable();
let should_log = log_level >= Level::Info;

// 使用 can_ 前缀
let can_edit = user.has_permission(Permission::Edit);
let can_delete = user.has_permission(Permission::Delete);
```

---

### 常量和静态变量命名

#### 基本规则

-   使用 `SCREAMING_SNAKE_CASE`（全大写下划线）
-   常量使用 `const` 关键字
-   静态变量使用 `static` 关键字

#### 示例

```rust
/// 最大重试次数
pub const MAX_RETRY_COUNT: u32 = 3;

/// 默认超时时间（秒）
pub const DEFAULT_TIMEOUT: u64 = 30;

/// 最大请求体大小（字节）
pub const MAX_REQUEST_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB

/// 应用名称
pub static APP_NAME: &str = "my-app";

/// 全局配置
pub static GLOBAL_CONFIG: Lazy<Config> = Lazy::new(|| {
    Config::load().expect("Failed to load config")
});
```

---

### 模块命名

#### 基本规则

-   使用 `snake_case` 命名
-   模块名应该是名词或名词短语
-   避免缩写，除非是广泛使用的

#### 示例

```rust
// 文件：src/plugin/service.rs
//! 服务依赖注入模块

// 文件：src/config/registry.rs
//! 配置注册表模块

// 文件：src/error/handler.rs
//! 错误处理模块
```

---

### 类型和 Trait 命名

#### 基本规则

-   使用 `PascalCase` 命名
-   Trait 名称通常是形容词或名词
-   类型别名使用 `PascalCase`

#### 示例

```rust
/// Service trait：支持依赖注入的特殊组件
pub trait Service: Clone + Sized + 'static {
    // trait 方法
}

/// 组件注册表 trait
pub trait ComponentRegistry {
    // trait 方法
}

/// 配置注册表 trait
pub trait ConfigRegistry {
    // trait 方法
}

/// 用户 ID 类型别名
pub type UserId = u64;

/// 结果类型别名
pub type AppResult<T> = Result<T, AppError>;
```

---

### 生命周期和泛型参数命名

#### 生命周期参数

-   使用小写字母，通常从 `'a` 开始
-   `'static` 是特殊的内置生命周期

```rust
/// 获取字符串引用
pub fn get_string<'a>(s: &'a str) -> &'a str {
    s
}

/// 结构体生命周期参数
pub struct Context<'a> {
    pub data: &'a str,
}
```

#### 泛型参数

-   单字母泛型：`T`（类型）、`E`（错误）、`R`（结果/注册表）
-   多字母泛型：使用 `PascalCase`，如 `Registry`, `Component`

```rust
/// 泛型服务构建函数
pub fn build<T, R>(registry: &R) -> Result<T>
where
    T: Service,
    R: ComponentRegistry + ConfigRegistry,
{
    // 实现
}

/// 泛型容器
pub struct Container<T> {
    pub value: T,
}

/// 带多个泛型参数的结构体
pub struct ServiceBuilder<Component, Config> {
    pub component: Component,
    pub config: Config,
}
```

---

### 命名规范检查清单

#### 结构体和类型

-   [ ] 使用 `PascalCase` 命名
-   [ ] 名称清晰描述类型用途
-   [ ] 避免不必要的缩写

#### 函数和变量

-   [ ] 使用 `snake_case` 命名
-   [ ] 函数名是动词或动词短语
-   [ ] 变量名是名词或名词短语
-   [ ] 布尔变量使用 `is_`, `has_`, `should_` 等前缀

#### 结构体字段

-   [ ] 使用 `snake_case` 命名
-   [ ] 字段名清晰描述用途
-   [ ] 布尔字段使用 `is_`, `has_` 等前缀
-   [ ] 序列化时如需要，使用 `serde(rename)` 属性

#### 常量和静态变量

-   [ ] 使用 `SCREAMING_SNAKE_CASE` 命名
-   [ ] 名称清晰描述常量用途

#### 模块

-   [ ] 使用 `snake_case` 命名
-   [ ] 模块名清晰描述模块功能

#### 通用要求

-   [ ] 遵循 Rust 官方命名约定
-   [ ] 名称具有描述性，避免单字母变量（除非是泛型参数）
-   [ ] 避免使用 Rust 关键字作为标识符
-   [ ] 保持命名一致性

---

### 命名规范示例：完整代码

```rust
//! 服务依赖注入模块
//!
//! 本模块实现了编译时依赖注入功能。

use crate::app::AppBuilder;
use crate::error::Result;

/// Service trait：支持依赖注入的特殊组件
pub trait Service: Clone + Sized + 'static {
    /// 从注册表构建服务组件
    fn build<R>(registry: &R) -> Result<Self>
    where
        R: ComponentRegistry + ConfigRegistry;
}

/// 中间件配置结构体
///
/// 用于配置应用的各种中间件组件。
#[derive(Debug, Clone, Deserialize)]
pub struct Middlewares {
    /// 启用压缩中间件
    pub compression: Option<EnableMiddleware>,

    /// 限制请求体大小中间件
    pub limit_payload: Option<LimitPayloadMiddleware>,

    /// 追踪日志中间件
    pub logger: Option<TraceLoggerMiddleware>,

    /// 静态资源服务中间件
    ///
    /// 注意：序列化时字段名为 `static`
    #[serde(rename = "static")]
    pub static_assets: Option<StaticAssetsMiddleware>,
}

/// 自动注入所有服务
///
/// 查找所有已注册的 `ServiceRegistrar` 并将它们安装到应用中。
///
/// # 参数
///
/// - `app`: 应用构建器
///
/// # 返回值
///
/// 如果所有服务都成功安装，返回 `Ok(())`；否则返回错误
pub fn auto_inject_service(app: &mut AppBuilder) -> Result<()> {
    // 收集所有服务注册器
    let registrars: Vec<&'static &dyn ServiceRegistrar> =
        inventory::iter::<&dyn ServiceRegistrar>().collect();

    let total = registrars.len();
    let mut pending: Vec<&'static &dyn ServiceRegistrar> = registrars;
    let mut installed = 0;
    let mut progress_made = false;

    // 循环安装服务，直到所有服务都安装成功
    while !pending.is_empty() {
        let mut next_pending = Vec::new();

        // 尝试安装每个待安装的服务
        for registrar in pending {
            match registrar.install_service(app) {
                Ok(()) => {
                    installed += 1;
                    progress_made = true;
                }
                Err(_) => {
                    next_pending.push(registrar);
                }
            }
        }

        // 如果一轮中没有进展，说明存在循环依赖或缺少依赖
        if !progress_made && !next_pending.is_empty() {
            return Err("Circular dependency or missing dependency".into());
        }

        pending = next_pending;
    }

    Ok(())
}

/// 最大重试次数
pub const MAX_RETRY_COUNT: u32 = 3;

/// 默认超时时间（秒）
pub const DEFAULT_TIMEOUT: u64 = 30;
```

---

## 📚 相关资源

-   [Rust 官方文档：注释](https://doc.rust-lang.org/book/ch14-02-publishing-to-crates-io.html#making-useful-documentation-comments)
-   [Rust 官方文档：文档注释](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html)
-   [Rust API 指南：文档](https://rust-lang.github.io/api-guidelines/documentation.html)

---

## 🔄 版本历史

-   **v1.2.0** (2024-01-XX): 添加复杂函数人性化注释指南，包含编写原则、对比示例、模板和常见术语通俗解释
-   **v1.1.0** (2024-01-XX): 添加变量命名规范章节，包含结构体、函数、变量、常量等命名约定
-   **v1.0.0** (2024-01-XX): 初始版本，基于 `mini-rs-v0.0.1` 项目总结

---

**💡 提示**：良好的注释是代码可维护性的重要保障。遵循本规范，让代码更易读、易懂、易维护。
