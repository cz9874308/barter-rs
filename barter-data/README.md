# Barter-Data

一个用于从领先的加密货币交易所流式传输公共市场数据的高性能 WebSocket 集成库 - 开箱即用。特点：

-   **简单**：Barter-Data 的简单 `StreamBuilder` 接口允许轻松快速设置（请参阅下面的示例！）。
-   **标准化**：Barter-Data 的统一接口用于消费公共 WebSocket 数据，意味着每个交易所都返回标准化的数据模型。
-   **实时**：Barter-Data 利用实时 WebSocket 集成，能够消费标准化的逐笔数据。
-   **可扩展**：Barter-Data 高度可扩展，因此通过编写新的集成可以轻松贡献！

**请参阅：[`Barter`]、[`Barter-Instrument`]、[`Barter-Execution`] 和 [`Barter-Integration`] 以获取其他 Barter 库的完整文档。**

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]
[![Discord chat][discord-badge]][discord-url]

[crates-badge]: https://img.shields.io/crates/v/barter-data.svg
[crates-url]: https://crates.io/crates/barter-data
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-data-rs/-/blob/main/LICENCE
[discord-badge]: https://img.shields.io/discord/910237311332151317.svg?logo=discord&style=flat-square
[discord-url]: https://discord.gg/wE7RqhnQMV

[API Documentation] | [Chat]

[`Barter`]: https://crates.io/crates/barter
[`Barter-Instrument`]: https://crates.io/crates/barter-instrument
[`Barter-Execution`]: https://crates.io/crates/barter-execution
[`Barter-Integration`]: https://crates.io/crates/barter-integration
[API Documentation]: https://docs.rs/barter-data/latest/barter_data
[Chat]: https://discord.gg/wE7RqhnQMV

## 概述

Barter-Data 是一个用于从领先的加密货币交易所流式传输公共市场数据的高性能 WebSocket 集成库。它提供了一套易于使用且可扩展的接口，可以实时提供标准化的交易所数据。

从用户的角度来看，主要组件是 `StreamBuilder` 结构，它有助于使用输入的 `Subscription` 初始化任意数量的交易所 `MarketStream`。只需构建您理想的 `MarketStreams` 集合，`Barter-Data` 将完成其余工作！

### 支持的交易所订阅

|         交易所          |           构造函数代码           |                  工具类型                   |                     订阅类型                     |
| :---------------------: | :------------------------------: | :-----------------------------------------: | :----------------------------------------------: |
|     **BinanceSpot**     |     `BinanceSpot::default()`     |                    Spot                     | PublicTrades <br> OrderBooksL1 <br> OrderBooksL2 |
|  **BinanceFuturesUsd**  |  `BinanceFuturesUsd::default()`  |                  Perpetual                  | PublicTrades <br> OrderBooksL1 <br> OrderBooksL2 |
|      **Bitfinex**       |            `Bitfinex`            |                    Spot                     |                   PublicTrades                   |
|       **Bitmex**        |             `Bitmex`             |                  Perpetual                  |                   PublicTrades                   |
|      **BybitSpot**      |      `BybitSpot::default()`      |                    Spot                     |                   PublicTrades                   |
| **BybitPerpetualsUsd**  | `BybitPerpetualsUsd::default()`  |                  Perpetual                  |                   PublicTrades                   |
|      **Coinbase**       |            `Coinbase`            |                    Spot                     |                   PublicTrades                   |
|     **GateioSpot**      |     `GateioSpot::default()`      |                    Spot                     |                   PublicTrades                   |
|  **GateioFuturesUsd**   |  `GateioFuturesUsd::default()`   |                   Future                    |                   PublicTrades                   |
|  **GateioFuturesBtc**   |  `GateioFuturesBtc::default()`   |                   Future                    |                   PublicTrades                   |
| **GateioPerpetualsUsd** | `GateioPerpetualsUsd::default()` |                  Perpetual                  |                   PublicTrades                   |
| **GateioPerpetualsBtc** | `GateioPerpetualsBtc::default()` |                  Perpetual                  |                   PublicTrades                   |
|  **GateioOptionsBtc**   |    `GateioOptions::default()`    |                   Option                    |                   PublicTrades                   |
|       **Kraken**        |             `Kraken`             |                    Spot                     |          PublicTrades <br> OrderBooksL1          |
|         **Okx**         |              `Okx`               | Spot <br> Future <br> Perpetual <br> Option |                   PublicTrades                   |

## 示例

请参阅 barter-data-rs/examples 以获取更全面的示例选择！

### 多交易所公共交易

```rust,no_run
use barter_data::{
    exchange::{
        binance::{futures::BinanceFuturesUsd, spot::BinanceSpot},
        bitmex::Bitmex,
        bybit::{futures::BybitPerpetualsUsd, spot::BybitSpot},
        coinbase::Coinbase,
        gateio::{
            option::GateioOptions,
            perpetual::{GateioPerpetualsBtc, GateioPerpetualsUsd},
            spot::GateioSpot,
        },
        okx::Okx,
    },
    streams::{Streams, reconnect::stream::ReconnectingStream},
    subscription::trade::PublicTrades,
};
use barter_integration::model::instrument::kind::{
    FutureContract, InstrumentKind, OptionContract, OptionExercise, OptionKind,
};
use chrono::{TimeZone, Utc};
use futures::StreamExt;

#[tokio::main]
async fn main() {
    // 为各种交易所初始化 PublicTrades 流
    // '--> 每次调用 StreamBuilder::subscribe() 都会创建一个单独的 WebSocket 连接
    let streams = Streams::<PublicTrades>::builder()
        .subscribe([
            (BinanceSpot::default(), "btc", "usdt", InstrumentKind::Spot, PublicTrades),
            (BinanceSpot::default(), "eth", "usdt", InstrumentKind::Spot, PublicTrades),
        ])
        .subscribe([
            (BinanceFuturesUsd::default(), "btc", "usdt", InstrumentKind::Perpetual, PublicTrades),
            (BinanceFuturesUsd::default(), "eth", "usdt", InstrumentKind::Perpetual, PublicTrades),
        ])
        .subscribe([
            (Coinbase, "btc", "usd", InstrumentKind::Spot, PublicTrades),
            (Coinbase, "eth", "usd", InstrumentKind::Spot, PublicTrades),
        ])
        .subscribe([
            (GateioSpot::default(), "btc", "usdt", InstrumentKind::Spot, PublicTrades),
        ])
        .subscribe([
            (GateioPerpetualsUsd::default(), "btc", "usdt", InstrumentKind::Perpetual, PublicTrades),
        ])
        .subscribe([
            (GateioPerpetualsBtc::default(), "btc", "usd", InstrumentKind::Perpetual, PublicTrades),
        ])
        .subscribe([
            (GateioOptions::default(), "btc", "usdt", InstrumentKind::Option(put_contract()), PublicTrades),
        ])
        .subscribe([
            (Okx, "btc", "usdt", InstrumentKind::Spot, PublicTrades),
            (Okx, "btc", "usdt", InstrumentKind::Perpetual, PublicTrades),
            (Okx, "btc", "usd", InstrumentKind::Future(future_contract()), PublicTrades),
            (Okx, "btc", "usd", InstrumentKind::Option(call_contract()), PublicTrades),
        ])
        .subscribe([
            (BybitSpot::default(), "btc", "usdt", InstrumentKind::Spot, PublicTrades),
            (BybitSpot::default(), "eth", "usdt", InstrumentKind::Spot, PublicTrades),
        ])
        .subscribe([
            (BybitPerpetualsUsd::default(), "btc", "usdt", InstrumentKind::Perpetual, PublicTrades),
        ])
        .subscribe([
            (Bitmex, "xbt", "usd", InstrumentKind::Perpetual, PublicTrades)
        ])
        .init()
        .await
        .unwrap();

    // 使用 futures_util::stream::select_all 选择和合并每个交易所的流
    // 注意：使用 `Streams.select(ExchangeId)` 与单个交易所流交互！
    let mut joined_stream = streams
        .select_all()
        .with_error_handler(|error| println!(format!("MarketStream generated error: {error:?}")));

    while let Some(event) = joined_stream.next().await {
        println!("{event:?}");
    }
}
```

## 获取帮助

首先，请查看[API 文档][API Documentation]中是否已有您问题的答案。如果找不到答案，我很乐意通过[聊天][Chat]在 Discord 上帮助您并尝试回答您的问题。

## 支持 Barter 开发

通过成为赞助商（或给我小费！）来帮助我们推进 Barter 的能力。

您的贡献将使我能够投入更多时间到 Barter，加速功能开发和改进。

**请发送邮件至 *justastream.code@gmail.com* 进行所有咨询**

更多信息请参阅[此处](../README.md#support-barter-development)。

## 贡献

提前感谢您帮助开发 Barter 生态系统！请通过 Discord [聊天][Chat]联系我们，讨论开发、新功能和未来路线图。

### 添加新的交易所连接器

1. 在 src/exchange/<exchange_name>.mod.rs 中添加新的 `Connector` trait 实现（例如，请参阅 exchange::okx::Okx）。
2. 按照下面的"为现有交易所连接器添加新的订阅类型"继续！

### 为现有交易所连接器添加新的订阅类型

1. 在 src/subscription/<sub_kind_name>.rs 中添加新的 `SubscriptionKind` trait 实现（例如，请参阅 subscription::trade::PublicTrades）。
2. 定义 `SubscriptionKind::Event` 数据模型（例如，请参阅 subscription::trade::PublicTrade）。
3. 定义交易所 `Connector` 将为新 `SubscriptionKind` 初始化的 `MarketStream` 类型：<br>
   即 `impl StreamSelector<SubscriptionKind> for <ExistingExchangeConnector> { ... }`
4. 尝试编译并按照剩余步骤操作！
5. 以标准格式添加 barter-data-rs/examples/<sub_kind_name>\_streams.rs 示例 :)

### 许可证

本项目采用 [MIT 许可证][MIT license]。

[MIT license]: https://github.com/barter-rs/barter-rs/blob/develop/LICENSE

### 贡献许可协议

您有意提交以包含在 Barter 工作空间 crate 中的任何贡献均应：

1. 采用 MIT 许可证
2. 受以下所有免责声明和责任限制的约束
3. 不提供任何附加条款或条件
4. 在理解仅用于教育目的和风险警告的前提下提交

通过提交贡献，您证明您有权根据这些条款这样做。

## 法律免责声明和责任限制

在使用本软件之前，请仔细阅读本免责声明。通过访问或使用本软件，您承认并同意受本条款的约束。

1. 教育目的
   本软件及相关文档（"软件"）仅用于教育和研究目的。本软件不适用于、未设计、未测试、未验证或未认证用于商业部署、实盘交易或任何形式的生产使用。

2. 非财务建议
   软件中包含的任何内容均不构成财务、投资、法律或税务建议。软件的任何方面都不应被依赖用于交易决策或财务规划。强烈建议用户咨询合格的专业人士，以获得适合其情况的投资指导。

3. 风险承担
   金融市场交易，包括但不限于加密货币、证券、衍生品和其他金融工具，存在重大损失风险。用户承认：
   a) 他们可能损失全部投资；
   b) 过往表现不代表未来结果；
   c) 假设或模拟的性能结果具有固有的局限性和偏差。

4. 免责声明
   本软件按"原样"提供，不提供任何形式的明示或暗示保证。在法律允许的最大范围内，作者和版权持有人明确否认所有保证，包括但不限于：
   a) 适销性
   b) 特定用途的适用性
   c) 不侵权
   d) 结果的准确性或可靠性
   e) 系统集成
   f) 安静享用

5. 责任限制
   在任何情况下，作者、版权持有人、贡献者或任何关联方均不对任何直接、间接、偶然、特殊、惩戒性或后果性损害（包括但不限于采购替代商品或服务、使用损失、数据或利润损失；或业务中断）承担责任，无论因何原因引起，也无论基于任何责任理论，无论是合同、严格责任还是侵权（包括疏忽或其他），即使已被告知此类损害的可能性。

6. 监管合规
   本软件未在任何金融监管机构注册、认可或批准。用户全权负责：
   a) 确定其使用是否符合适用的法律法规
   b) 获得任何所需的许可证、许可或注册
   c) 满足其管辖范围内的任何监管义务

7. 赔偿
   用户同意赔偿、辩护并使作者、版权持有人和任何关联方免受因使用本软件而产生的任何索赔、责任、损害、损失和费用。

8. 确认
   通过使用本软件，用户确认已阅读本免责声明，理解并同意受其条款和条件的约束。

上述限制可能不适用于不允许排除某些保证或限制责任的司法管辖区。
