# Portal 用户面板

拼车用户控制面平行于管理员，不改变 `/v1` 协议。HTTP 字段见 [接口文档](api.md#portal-用户面板)。

- 用户面：`/portal`、`/api/portal/*`，Cookie `cpr_portal_session`
- 管理面仍为 `/` 与 `/api/admin/*`；用户/套餐管理在 `/portal-users`、`/portal-plans`
- 密钥仍是 `client_api_keys`，请求仍是 `model_requests`（冻结 `owner_user_id`）
- 日/周 USD 与并发/RPM 按用户合计，同时受 Key 自身限额约束
- 用户 Key 禁止无分组（空池，不回退全部账号）
- 用户只禁用不物理删除；改密和停用会在同一事务里撤销会话
- 独立迁移目录 `backend/crates/gateway-store/migrations/portal/`，冻结清单为同目录 `.frozen-sha256`
- 在线更新默认关闭；需要时设置 `CPR_ENABLE_UPDATES=true`
