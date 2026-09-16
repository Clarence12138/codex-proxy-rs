# 客户端请求捕获样本

这里是回归测试输入及生成方法，不是生产请求转储，也不提供客户端不可识别保证。
所有登录 token、账号、会话和用户内容均为合成数据；未使用真实账号配置。
fixture 保留请求头和 JSON 字段顺序。HTTP body 保存为解压后的 JSON，Rust 回放时按原头重建入站压缩。

## Pi 0.85.1

`pi-0.85.1.json` 通过真实 `pi-ai` provider 与 `pi-coding-agent` 系统提示词构造器生成，
源信息记录 OpenAI SDK、Node 和 undici 版本。覆盖六组、十二轮：

- `openai-responses` HTTP/SSE；
- `openai-codex-responses` HTTP/SSE；
- `openai-codex-responses` WebSocket；
- 每条路径分别使用默认提示词与显式自定义提示词；每组均完成工具调用、合成工具结果和最终文本。

捕获使用 mock fetch 与 loopback WebSocket。WS 使用 Pi Node CLI 所用 undici 的 WebSocket 实现，
没有替换 provider 的请求编码；构造器限制为唯一的本地地址，WS 失败不得悄悄回退 HTTP。
不启动 CLI 的登录、模型发现、项目资源加载或更新检查，不读取 `auth.json` / `models.json`。
`PI_PACKAGE_DIR` 与工作目录仅在捕获进程中设为合成路径，host 端口与 WS 随机 key 被规范化。

显式提供已安装的 **0.85.1** 程序包目录；脚本不下载或安装依赖，不依赖维护者的个人目录：

```bash
node backend/apps/gateway/tests/fixtures/client_requests/capture_pi.mjs \
  "$PI_AI_DIR" "$PI_CODING_AGENT_DIR" \
  backend/apps/gateway/tests/fixtures/client_requests/pi-0.85.1.json
```

默认提示词中的 Pi 品牌和文档块**应当仍然存在**；自定义提示词不自动加入它们。
第二轮工具结果特意包含 `hello pi`，用于证明网关没有全文清除品牌字符串。
这里的工具由 mock 模型选择，不代表真实模型质量或线上兼容性验收。

## Codex Desktop 基准

`desktop-26.908.40834.json` 对应：

| 项目 | 核实值 |
| --- | --- |
| App 版本 / build | `26.908.40834` / `8881` |
| 制品 | 官方 macOS arm64 ZIP，内部顶层 bundle 名为 `ChatGPT.app` |
| bundled Core `--version` | `codex-cli 0.154.0-alpha.6.2` |
| `Contents/Resources/codex` SHA-256 | `ecad78dbf98adb89ec475edac86630406cbe59d9f3070b17d88065f136b94bcb` |
| `Contents/Resources/app.asar` SHA-256 | `bb40cd8811887363104a19291346af9595632e0e956316a1086b274fb8e3eafc` |
| 同版本开源 Core commit | [`b5bffd3ec4db487e7e3dec59663875b0ef7b72ca`](https://github.com/openai/codex/tree/b5bffd3ec4db487e7e3dec59663875b0ef7b72ca) |

`codex-cli` 是该内嵌可执行文件的版本输出名称，不代表样本来自独立安装的 CLI。
不要把它简化成稳定版 `0.154.0`，也不要据此更改生产默认画像或旧 TLS 测试期望。

证据分层：

1. **公开制品核实**：ZIP `Info.plist`、Core 二进制、`app.asar`。
   `.vite/build/main-DaMR-wdT.js` 使用 Desktop 名称及应用版本构造 `clientInfo`；
   `window-all-closed-BxbCP6YG.js` 的 `tt` 导出为 `Codex Desktop`；
   `src-CCXHtyvY.js` 设置 `CODEX_INTERNAL_ORIGINATOR_OVERRIDE`，并把 clientInfo 送入 initialize。
2. **bundled app-server 运行捕获**：使用上述身份执行 initialize；本地真实捕获 HTTP 的 zstd body，
   以及 WS opening、`generate=false` 预热和带 `previous_response_id` 的续接帧。
   UA 的运行系统信息来自捕获主机，固定保存在 fixture 测试画像，不改变生产默认值。
3. **开源 Core 核对**：同版本
   [默认身份构造](https://github.com/openai/codex/blob/b5bffd3ec4db487e7e3dec59663875b0ef7b72ca/codex-rs/login/src/auth/default_client.rs)
   及 [OpenAI provider](https://github.com/openai/codex/blob/b5bffd3ec4db487e7e3dec59663875b0ef7b72ca/codex-rs/model-provider-info/src/lib.rs)。
4. **未验证**：完整 Electron GUI、实际设备证明、真实上游授权与风控、当前最新版 App 的完整行为。
   本地捕获显式禁用 analytics/feedback 和 attestation，使用合成 ChatGPT external tokens、
   空本地模型目录及合成 base/developer instructions；这些是测试条件，不是 App 默认行为的断言。

需要 macOS arm64、Python 3.14（标准库 zstd）及 `/usr/bin/sandbox-exec`。
先自行取得上述官方 ZIP，脚本只接受本地文件并检查固定版本和哈希，不安装或替换系统 App：

```bash
python3 backend/apps/gateway/tests/fixtures/client_requests/capture_desktop.py \
  /path/to/ChatGPT-darwin-arm64-26.908.40834.zip \
  backend/apps/gateway/tests/fixtures/client_requests/desktop-26.908.40834.json
```

制品地址记录在 JSON 的 `source.artifact_url`，可从
[官方 appcast](https://persistent.oaistatic.com/codex-app-prod/appcast.xml) 核对。
脚本创建独立 HOME/CODEX_HOME，并**同时**设置 `openai_base_url` 和 `chatgpt_base_url`：
只设置后者不会改变模型端点。系统沙箱仅放行指定 loopback 出站端口，其他 HTTP/CONNECT 尝试
由本地拒绝代理拦截、绝不转发；被拒绝的辅助目标单独记录在样本中。
SSE 通过本地 WS 426 触发 Core 自身 fallback；WS 服务不协商压缩，此样本不能替代现有 deflate/TLS 指纹测试。

## Rust 回放与验收边界

从仓库根目录运行：

```bash
RUST_MIN_STACK=16777216 cargo test --manifest-path backend/Cargo.toml \
  -p codex-proxy-rs --test main --locked wire_replay
```

常规 CI 不要求 Pi、Desktop 二进制、数据库或外网模型服务。
组合根测试串联真实 API decoder → OpenAI encoder → transport → loopback HTTP/WS server，
检查最终请求头、zstd 解压 body、WS 帧及响应终态，并对 Pi 的两轮工具协议做回放。
额外的对抗样本添加重复环境头和未知业务字段，原始样本也独立回放。

此处绕过认证、账号选择、持久化和计费，只注入合成上游账号上下文；不是完整网关业务端到端验收。
API 的 CORS/原始客户端观测和 Provider 非 HTTP 编码入口在各自 crate 测试中覆盖。
未知业务头、trace headers、项目指令、工作目录、工具和提示词仍可透露环境；移除 SDK 头不等于匿名化。
