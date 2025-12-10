# Barter-Execution

从金融场所流式传输私有账户数据，并执行（实盘或模拟）订单。还提供了功能丰富的 MockExchange 和 MockExecutionClient 以协助回测和模拟交易。

**特点：**

-   **简单**：`ExecutionClient` trait 提供了与交易所交互的统一且简单的接口。
-   **标准化**：允许您的策略使用相同的接口与每个真实或模拟交易所通信。
-   **可扩展**：Barter-Execution 高度可扩展，通过添加新的交易所集成可以轻松贡献！

**请参阅：[`Barter`]、[`Barter-Data`]、[`Barter-Instrument`] 和 [`Barter-Integration`] 以获取其他 Barter 库的完整文档。**

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]
[![Discord chat][discord-badge]][discord-url]

[crates-badge]: https://img.shields.io/crates/v/barter-execution.svg
[crates-url]: https://crates.io/crates/barter-execution
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-execution-rs/-/blob/main/LICENCE
[discord-badge]: https://img.shields.io/discord/910237311332151317.svg?logo=discord&style=flat-square
[discord-url]: https://discord.gg/wE7RqhnQMV

[API Documentation] | [Chat]

[`Barter`]: https://crates.io/crates/barter
[`Barter-Data`]: https://crates.io/crates/barter-data
[`Barter-Instrument`]: https://crates.io/crates/barter-instrument
[`Barter-Integration`]: https://crates.io/crates/barter-integration
[barter-examples]: https://github.com/barter-rs/barter-rs/tree/develop/barter/examples
[API Documentation]: https://docs.rs/barter-execution/latest/barter_execution
[Chat]: https://discord.gg/wE7RqhnQMV

## 概述

高性能和标准化的交易接口，能够在多个金融场所执行交易。还提供了功能丰富的模拟交易所，以协助回测和干运行交易。通过初始化其关联的 `ExecutionClient` 实例与交易所通信。

## 示例

-   请参阅[此处][barter-examples]查看 Barter-Instrument 的实际使用示例。
-   请参阅其他子模块以获取每个库的更多示例。

## 获取帮助

首先，请查看[API 文档][API Documentation]中是否已有您问题的答案。如果找不到答案，我很乐意通过[聊天][Chat]在 Discord 上帮助您并尝试回答您的问题。

## 支持 Barter 开发

通过成为赞助商（或给我小费！）来帮助我们推进 Barter 的能力。

您的贡献将使我能够投入更多时间到 Barter，加速功能开发和改进。

**请发送邮件至 *justastream.code@gmail.com* 进行所有咨询**

更多信息请参阅[此处](../README.md#support-barter-development)。

## 贡献

提前感谢您帮助开发 Barter 生态系统！请通过 Discord [聊天][Chat]联系我们，讨论开发、新功能和未来路线图。

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
