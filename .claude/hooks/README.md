# Claude Code Hooks for Memex-CLI Memory Service

这个目录包含了用于集成 Memex-CLI 记忆服务与 Claude Code 的 Hook 脚本。

## 文件说明

### Hook 脚本

1. **memory-inject.py** - 记忆检索与上下文注入
   - 触发时机: `UserPromptSubmit`
   - 功能: 用户提交提示词时自动检索相关历史记忆并注入上下文
   - 超时: 10秒

2. **memory-record.py** - 工具使用记录
   - 触发时机: `PostToolUse`
   - 功能: 在 Write/Edit/Bash 工具执行后自动记录知识
   - 超时: 5秒
   - 记录的工具: Write, Edit, Bash

3. **session-summarize.py** - 会话知识汇总
   - 触发时机: `Stop`
   - 功能: Claude 完成任务时批量提取和保存会话知识
   - 超时: 30秒

### 配置文件

- **settings.json** - Hook 配置
  - 位于父目录 `.claude/settings.json`
  - 定义了三个 Hook 的触发条件和命令

### 日志文件

- `memory-inject.log` - 记忆注入日志
- `memory-record.log` - 记忆记录日志
- `session-summarize.log` - 会话总结日志

## 安装步骤

### 1. 编译 memex-cli

```bash
cd /path/to/memex_cli
cargo build --release -p memex-cli

# 将可执行文件添加到 PATH
# Windows: 复制 target/release/memex-cli.exe 到某个 PATH 目录
# Linux/macOS:
# sudo cp target/release/memex-cli /usr/local/bin/
```

### 2. 配置记忆服务

编辑 `~/.memex/config.toml` (如果不存在则创建):

```toml
[memory]
base_url = "http://localhost:8080"  # 你的记忆服务 API 地址
api_key = "your-api-key-here"       # API 密钥
timeout_ms = 30000

[memory.search]
limit = 5
min_score = 0.6
```

### 3. 设置 Python 环境

确保 Python 3.8+ 已安装并在 PATH 中：

```bash
python --version  # 应显示 3.8 或更高版本
```

### 4. 使 Hook 脚本可执行 (Linux/macOS)

```bash
chmod +x .claude/hooks/*.py
```

### 5. 测试 Hook 配置

在项目目录启动 Claude Code，运行：

```bash
claude --debug
```

检查输出中是否有 Hook 注册信息。

## 使用方法

### 自动记忆检索

当你向 Claude 提问时，Hook 会自动：
1. 检索相关历史记忆
2. 将记忆格式化为 Markdown
3. 注入到对话上下文
4. Claude 看到增强后的上下文进行回答

示例：
```
你：如何实现 JWT 认证？

[自动注入的记忆]
### 📚 相关历史记忆

**[qa-12345]** Q: JWT 认证最佳实践
A: 使用 httpOnly cookie 存储 token...
相关性: 0.85
---

Claude: 基于之前的经验 [qa-12345]，我建议...
```

### 自动记忆记录

当 Claude 执行工具时，Hook 会自动：
1. 捕获工具使用信息
2. 提取查询和答案
3. 调用 memex-cli 记录候选知识

记录的工具：
- **Write**: 文件创建
- **Edit**: 文件编辑
- **Bash**: 命令执行（跳过简单命令）

### 会话知识汇总

当 Claude 完成任务时，Hook 会：
1. 读取完整会话转录
2. 提取关键知识点
3. 批量保存到记忆服务

## 手动测试

### 测试记忆检索

```bash
cd /path/to/memex_cli
memex-cli search --query "JWT 认证" --limit 5 --min-score 0.6 --format json
```

### 测试记忆记录

```bash
memex-cli record-candidate \
  --query "实现用户认证" \
  --answer "创建了 auth.ts 实现 JWT 认证" \
  --tags "typescript,auth,jwt"
```

### 测试 Hook 脚本

模拟 UserPromptSubmit Hook 输入：

```bash
echo '{"prompt":"如何实现 JWT 认证？","session_id":"test-123"}' | \
  python .claude/hooks/memory-inject.py
```

## 故障排查

### Hook 未触发

1. 检查 settings.json 格式是否正确
2. 运行 `claude --debug` 查看详细日志
3. 检查日志文件 (*.log)

### memex-cli 找不到

编辑 Hook 脚本，使用绝对路径：

**Windows:**
```python
memex_cli = r"C:\path\to\memex-cli.exe"
```

**Linux/macOS:**
```python
memex_cli = "/usr/local/bin/memex-cli"
```

### 记忆服务连接失败

1. 检查 ~/.memex/config.toml 中的 base_url
2. 确认记忆服务正在运行
3. 测试连接: `curl http://localhost:8080/health`

### Python 找不到

在 settings.json 中使用绝对路径：

```json
{
  "command": "C:\\Python310\\python.exe .claude/hooks/memory-inject.py"
}
```

## 配置调整

### 修改检索参数

编辑 `memory-inject.py`:

```python
cmd = [
    memex_cli, "search",
    "--query", user_prompt,
    "--limit", "10",        # 增加结果数量
    "--min-score", "0.5",   # 降低相关性阈值
    "--format", "json"
]
```

### 禁用特定 Hook

编辑 `.claude/settings.json`，删除或注释相应的 Hook 配置。

临时禁用所有 Hook:
```bash
mv .claude/settings.json .claude/settings.json.disabled
```

### 自定义记录逻辑

编辑 `memory-record.py` 的 `RECORDABLE_TOOLS` 列表：

```python
# 只记录 Write 工具
RECORDABLE_TOOLS = ["Write"]

# 或添加更多工具
RECORDABLE_TOOLS = ["Write", "Edit", "Bash", "Read", "Grep"]
```

## 性能优化

### 缓存搜索结果

在 `memory-inject.py` 中添加简单的 LRU 缓存：

```python
from functools import lru_cache

@lru_cache(maxsize=100)
def search_memory(query):
    # ... 搜索逻辑
```

### 异步执行

对于耗时操作，使用后台进程：

```python
# 在 memory-record.py 中
subprocess.Popen(
    cmd,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL
)
```

## 安全注意事项

1. **敏感信息过滤**: Hook 脚本不记录包含密码、API 密钥的内容
2. **日志轮转**: 定期清理 .log 文件避免占用过多空间
3. **权限控制**: Hook 脚本仅读取必要文件，不修改系统配置

## 进一步定制

### 添加自定义标签

在 `memory-record.py` 中添加更多元数据：

```python
cmd = [
    memex_cli, "record-candidate",
    "--query", query,
    "--answer", answer,
    "--tags", f"tool:{tool_name.lower()},project:myproject,auto",
    "--metadata", json.dumps({"timestamp": "2026-01-08", "user": "me"})
]
```

### 集成其他工具

可以在 Hook 脚本中调用其他工具：

```python
# 使用 GitHub API 记录 PR 相关信息
# 使用 Jira API 关联 Issue
# 使用自定义分析工具提取知识
```

## 相关资源

- [Memex-CLI 文档](../CLAUDE.md)
- [Claude Code Hooks 官方文档](https://code.claude.com/docs/en/hooks)
- [记忆服务架构分析](../memory-service-hook-analysis.md)

## 支持

如遇问题，请检查：
1. 日志文件 (.claude/hooks/*.log)
2. Claude Code 调试输出 (`claude --debug`)
3. Memex-CLI 日志 (RUST_LOG=debug memex-cli ...)

---

**版本**: v1.0
**最后更新**: 2026-01-08
