# 二开目标与差异

## 目标

在保留上游对原生 Codex 请求头、指纹、协议等核心设计的基础上扩展拼车能力，以增强拼车可用性为目的，尽量不破坏上游核心设计。

## 设计边界

优先使用既有模块和扩展点，不为局部拼车能力无依据地侵入上游核心链路。核心设计并非绝对不可修改：确需偏离时，先说明拼车场景、实际约束、兼容影响和验证依据；涉及实质性设计取舍时与用户确认。

涉及上游行为时核对对应版本的官方实现；内部变更依据本项目代码、合同与测试。模块职责仍以 [系统架构](architecture.md) 为准，不在本文重复维护架构规则。

## 开发与贡献分支

- `main` 专门作为上游同步基线，不承载二开提交或本 fork 的个性化配置；保持与所同步的上游提交一致，不将 `custom` 的改动反向合入。
- `custom` 是本 fork 的主要开发与贡献分支，承载拼车扩展及本 fork 的协作约定。个人允许直接开发和推送，不强制功能分支或 PR；外部贡献默认向 `custom` 提 PR。

向上游贡献时核对上游自己的分支与协作规则。这里约定分支用途，不代表 GitHub 默认分支或保护设置已完成配置。

允许直接推送是工作方式，不是每次修改后自动提交或推送的授权。提交、推送、PR、合并、发版和生产操作仍按当前明确授权执行，详见 [贡献与审查](../CONTRIBUTING.md#pr-流程)。开发分支不等同于已验证的发布分支，发版时另行核对实际脚本与工作流。

## 上游同步

定期将选定的上游基线同步到 `main`，再评估将合理更新集成到 `custom`，检查与拼车扩展的冲突、协议兼容性和升级影响。不盲目全量接受上游变更，也不以同步为由覆盖本地有意保留的设计；同步范围、集成方式与实质性取舍按实际任务确认，不默认改写已发布历史。

上游同步可能重新带入其协作规则、分支假设或规则保护工作流；集成到 `custom` 时检查这些差异，保留本 fork 已确认的约定，不为此改写 `main` 上的上游文件。

## 镜像交付

本 fork 的 [CI](../.github/workflows/ci.yml) 在 `custom` 的应用或容器相关变更上自动执行质量检查、构建 `linux/amd64` 镜像并发布到 `ghcr.io/clarence12138/codex-proxy-rs`。后端、前端、部署文件、版本元数据、容器验证 action 与构建/质量工作流变更属于此范围；根协作文档、`docs/` 和 `.local/` 不触发镜像构建。源码目录内部仍按目录范围检查。

发布必须通过完整后端、前端和 workflow lint 检查，以及同一镜像的漏洞与运行验证。`main` 和任何 PR 只检查、不发布；其他仓库或其他 ref 也不能发布到本 fork 的镜像地址。手动运行 CI 会强制完整检查和构建，但仍只有本 fork 的 `custom` 可以发布。

- 定位源码的 tag：`ghcr.io/clarence12138/codex-proxy-rs:custom-<完整 commit SHA>`。
- 固定部署引用：`ghcr.io/clarence12138/codex-proxy-rs@sha256:<manifest digest>`，从成功运行的摘要中取得。
- 不发布浮动 `latest`。同一 commit 重新构建可能因系统安全更新生成不同 digest，所以 SHA tag 不等于不可变产物。
- 工作流先构建并加载一次镜像，验证后推送同一产物，再核对 registry 引用与镜像 ID；失败时不提供已验证部署引用。
- 构建发布不等于部署，不提供生产 SSH 凭据、不自动上线。生产仅拉取已验证镜像，更新步骤见 [镜像升级与源码构建](../deploy/README.md#镜像升级与源码构建)。

### 首次公开与手动入口

GHCR 首次发布的包默认私有，推送成功不能证明已公开。首次获授权推送并构建后，在包设置中将 `codex-proxy-rs` 设为 **Public**，确认关联 `Clarence12138/codex-proxy-rs` 及其 Actions 发布权限。已有同名包若未关联，需要先修正权限；不通过新增管理员 PAT 绕过。依据见 [GitHub Container registry 文档](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)。

随后使用没有登录缓存的 Docker 配置验证摘要中的精确 digest 可匿名拉取，例如在已授权的验证机器上执行以下模板（替换占位符）：

```bash
anonymous_config="$(mktemp -d)"
docker --config "$anonymous_config" pull 'ghcr.io/clarence12138/codex-proxy-rs@sha256:<manifest digest>'
rm -rf "$anonymous_config"
```

工作流摘要及 reusable outputs 将包可见性和匿名拉取标为 `not_verified`，需另行验收；认证推送和匿名 registry 请求均不能代替实际匿名下载。生产机器上的拉取和上线仍需对应授权。

`workflow_dispatch` 的发现与使用要求工作流存在于 GitHub 默认分支。建议将本 fork 默认分支设为 `custom`，而不是把二开工作流加到纯上游 `main`；这是独立的仓库设置操作，文档不代表设置已完成。`custom` push 自动运行不依赖该调整。使用 `gh` 时显式指定 `--repo Clarence12138/codex-proxy-rs`，避免本机默认定位到上游。

上游 [正式发版工作流](../.github/workflows/release.yml) 仍负责原有版本与多平台产物，不作为 custom 日常镜像交付入口；本流程不修改版本号、不创建正式 Release。

## 差异记录

只记录已核实且仍有效的重要差异，说明适用范围并链接所属模块的 API、架构或部署文档；实现过程与历史留在提交、Issue 或 PR 中。不把尚未实现的计划写成已有功能，不重复维护整套架构说明。

- Responses 不透传下游 `x-codex-installation-id` 请求头，避免与按所选账号写入的正文安装身份不一致；正文使用官方 `client_metadata["x-codex-installation-id"]` 键名。未知业务扩展和提示词不做品牌清洗，字段范围见 [API 合同](api.md#3-openai-数据面与模型目录)，Pi 自定义提示词是[客户端显式选择](../deploy/README.md#pi-接入与提示词边界)，不提供客户端不可识别保证。
- 在上游单 Key 用量页之外保留 [Portal 用户、套餐和订阅](portal.md)。用户概览跨其全部 Key 按请求冻结的用户归属聚合，Key 登录仍只查看单把 Key；两类面板复用展示组件，不共用权限范围。
- 用户并发/RPM 与日/周预算仍按用户合计，同时受 Key 自身限制。上游 Key/账号排队不新增用户队列、不扩大用户总限额；自定义模型价格及倍率继续保留。

本文不代表已完成全仓差异审计，也不声明具体线上版本或发布状态。
