## 退出码映射（CLI 层统一处理）

建议在 `mem-codecli` bin crate 中定义：

- `0`：成功（继承 codecli 的成功）
- `10`：CLI 参数错误 / 使用方式错误（clap 已处理可不需要）
- `11`：配置错误（ConfigError）
- `20`：Runner 启动/IO 错误
- `30`：Memory 服务错误（不可恢复或重试耗尽）
- `31`：Memory 未授权/鉴权失败
- `40`：Policy 拒绝（ToolDenied / 非交互审批失败）
- `50`：内部错误（InvalidState / 未分类）

映射规则（示例）：

- `MemoryError::Unauthorized` → `31`
- `MemoryError::is_retryable()==true` 且重试后仍失败 → `30`
- `CoreError::ToolDenied` 或 `PolicyError::NonInteractiveApprovalRequired` → `40`