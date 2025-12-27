## 1. 目标与关键约束

1. **实时透传**：子进程 stdout/stderr 必须以字节流实时转发到父进程对应输出。
2. **旁路采集**：同时捕获 stdout/stderr 的尾部（ring buffer，按字节上限），用于 gatekeeper/diagnostics。
3. **不阻塞**：tee/capture 不得阻塞主输出；写父输出慢时要有背压策略，但不能无限缓存导致 OOM。
4. **跨平台**：Unix 与 Windows 的信号/控制事件机制不同；实现要明确降级路径。

---

## 2. Tee 实现的结构化方案

### 2.1 核心思路：两个 reader task + 两个 writer（父输出）+ ring buffer

- spawn 子进程：`tokio::process::Command`
  - `stdin`：piped（用于 control JSONL）
  - `stdout`：piped
  - `stderr`：piped
- 启动两个异步 task：
  - `pump_stdout`: 读取 child stdout → 写 parent stdout → 写 ring buffer（stdout）
  - `pump_stderr`: 读取 child stderr → 写 parent stderr → 写 ring buffer（stderr）
- 主线程（或第三个 task）等待 child 退出：`child.wait().await`

> 建议：读写按 **chunk（Bytes）** 而不是按行；解析 JSONL 事件时可以另起一个“按行解析器”在 tee 后的副本数据上做（例如对 ring buffer 的增量流或 tee 中同时投递行缓冲），但 tee 本身不应强依赖行边界。

---

## 3. 具体 pump（tee）逻辑

### 3.1 推荐 I/O 模式：chunk pump + 有限缓冲 + 显式 flush（可选）

- 使用 `tokio::io::AsyncReadExt::read_buf` 从 child stdout/stderr 读 chunk。
- 对 parent stdout/stderr 用 `AsyncWriteExt::write_all` 写入。
- ring buffer 用自定义结构存最后 N 字节（见 3.3）。
- 写入 parent 输出慢时：
  - **策略 A（推荐）**：直接 await `write_all`（天然背压）。风险：parent 输出若被管道阻塞会拖慢；但这是符合“透传真实行为”的。
  - **策略 B（可选）**：使用 bounded channel，将读写解耦（reader 快、writer 慢时丢弃或阻塞）。除非你要“永不阻塞读取”，否则不必复杂化。

一般 CLI runner 采用策略 A 即可：实现简单、行为一致、可预期。

### 3.2 pump 伪代码（可直接翻译成 Rust）

```rust
async fn pump<R, W>(
    mut rd: R,
    mut wr: W,
    ring: Arc<Mutex<RingBytes>>,
    stream_enabled: bool,
    label: &'static str,
) -> Result<u64, RunnerError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 16 * 1024];
    let mut total: u64 = 0;

    loop {
        let n = rd.read(&mut buf).await
            .map_err(|e| RunnerError::StreamIo { stream: label, source: e })?;
        if n == 0 { break; }

        total += n as u64;

        // capture tail
        {
            let mut g = ring.lock().unwrap();
            g.push(&buf[..n]);
        }

        // stream passthrough
        if stream_enabled {
            wr.write_all(&buf[..n]).await
              .map_err(|e| RunnerError::StreamIo { stream: label, source: e })?;
            // flush：通常不必每次 flush；在交互性强的场景可按节流 flush
        }
    }

    Ok(total)
}
```

### 3.3 Ring buffer（尾部字节缓存）实现要点

目标：只保留最后 `max_capture_bytes`，避免 OOM，且 push 为 O(n) 的可接受实现。

简化实现（足够用）：

- `VecDeque<u8>` 存字节
- `push(bytes)`：逐字节 push\_back，并在超限时 pop\_front
- 最终输出：收集成 `Vec<u8>`

如果你追求性能，可用“循环数组 + head/tail”，但不是必须。

---

## 4. 等待退出与退出码归一化

### 4.1 正常退出码

- `status.code()`：Unix/Windows 都可用（Windows 总是 Some(code)）
- 如果 `None`（Unix 常见：被 signal 杀死），需要额外处理（见 4.2）。

### 4.2 Unix：signal 终止的归一化

- `std::os::unix::process::ExitStatusExt::signal()` 可取到 signal number。
- 归一化建议：
  - 若是 signal：返回 `128 + signal`（与多数 shell 约定一致）
  - 同时在结构化日志记录 `signal=<n>`，并将 RunnerError::Signal 作为可选信息（不一定要当 error）

> 注意：如果你需要严格复刻 `bash` 行为，`SIGINT` 通常 130，`SIGTERM` 143。

### 4.3 Windows：无 POSIX signal，只有退出码 + 控制事件

- Windows 没有 `signal()`；一般只拿退出码。
- 建议：直接返回 `status.code().unwrap_or(1)`。
- 对于“被强制终止”（TerminateProcess）也会表现为特定 code（实现/环境不同），不应依赖固定数值，只做日志记录即可。

---

## 5. 信号转发（用户 Ctrl+C / 终止）

信号转发的目标：用户终端对 `mem-codecli` 的中断能够传递给子进程，让子进程有机会清理并退出。

### 5.1 Unix（推荐完整实现）

建议做两件事：

1. **让 child 进入独立进程组**（可选但推荐）
   - 这样你可以对整个组发送信号（child 及其子孙进程）
2. **捕获 SIGINT/SIGTERM 并转发给 child（进程组）**

实现要点（tokio）：

- 用 `CommandExt::pre_exec` 或 `setsid`/`setpgid`（细节取决于你是否需要新会话）
- 捕获信号：
  - `tokio::signal::unix::signal(SignalKind::interrupt())`
  - `tokio::signal::unix::signal(SignalKind::terminate())`
- 转发：
  - `nix::sys::signal::killpg(pgid, Signal::SIGINT/SIGTERM)`（或 `libc::kill`）

行为建议：

- 第一次 SIGINT → 转发 SIGINT
- 若 `term_grace_ms` 后未退出 → 转发 SIGTERM
- 再未退出 → SIGKILL

> 若你不引入 `nix`，可直接用 `libc` 调用 `kill`/`killpg`。

### 5.2 Windows（必须接受差异与降级）

Windows 的 Ctrl+C 传递规则比较特殊：

- 默认情况下，控制台 Ctrl+C 会广播给同一控制台进程组中的进程，但 Rust/tokio 环境下是否能稳定转发取决于：
  - child 是否在同一控制台
  - child 是否在新进程组
  - 你的程序是否安装了 Ctrl handler

可落地方案（推荐优先级顺序）：

**方案 A（优先）：同控制台 + 让 child 在新进程组 + 发送 CTRL\_BREAK\_EVENT**

- spawn 时设置 `CREATE_NEW_PROCESS_GROUP`
- 捕获 `tokio::signal::ctrl_c()`
- 使用 Windows API `GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid_or_group)`
  - 实务上通常向进程组发送（group id = child pid）

注意点：

- `GenerateConsoleCtrlEvent` 要求发送方与接收方在同一控制台。
- `CTRL_C_EVENT` 往往会把自己也打断；使用 `CTRL_BREAK_EVENT` 更可控。

**方案 B（降级）：TerminateProcess**

- 如果无法生成控制事件（例如无控制台、权限问题、child 不同 console），就走“优雅失败”：
  - `child.kill().await`（tokio 会调用 TerminateProcess）
- 并在日志里标记 `signal_forward=unsupported`。

> 现实中，Windows 的“优雅中断”难以保证跨进程一致；建议在文档中明确：Windows 默认优先尝试 CTRL\_BREAK，失败则强制 kill。

---

## 6. 竞态与收尾：确保 pump task 正确结束

常见竞态：

- child 已退出，但 stdout/stderr 还未读完（缓冲区有剩余）
- stdout/stderr reader 结束（EOF），但 wait 还没返回（极少见，但可发生）

推荐收尾策略：

1. `wait_task = child.wait()`
2. `stdout_task = pump_stdout`
3. `stderr_task = pump_stderr`
4. 用 `tokio::try_join!` 等待三者完成
   - 但注意：如果你实现了“强杀”逻辑，wait\_task 可能需要被驱动完成

示意：

- 正常路径：`join!(stdout_task, stderr_task, wait_task)`
- 中断路径：先发信号/kill，再 await `wait_task`，然后等两条 pump 读到 EOF 自行退出

---

## 7. Windows 兼容清单（你实现时按表逐项对齐）

1. **行结束符**：Windows 输出可能是 `\r\n`；你若做按行解析 JSONL，要在 parser 中 `trim_end_matches('\r')`。
2. **控制台编码**：某些环境 stderr/stdout 不是 UTF-8。tee 应按字节透传；只有 gatekeeper 需要字符串时再做“尽力 UTF-8 解码（lossy）”。
3. **Ctrl 事件可用性**：无控制台（CI、服务）时 `GenerateConsoleCtrlEvent` 不可靠；必须降级 kill。
4. **进程树终止**：Unix 可 killpg；Windows TerminateProcess 不会自动杀子孙进程。若你必须“杀全树”，需要 Job Object（增强项）。MVP 可不做，但要在文档里说明限制。
5. **exit code 语义**：不要假设特定 code 表示 signal；以日志记录为主。

---

## 8. 推荐的实现分层（便于测试）

- `tee::pump()`：纯 I/O（可用内存流单测）
- `ring::RingBytes`：容量限制单测
- `process::spawn_codecli()`：平台差异封装（unix/windows cfg）
- `signals::forwarder()`：平台差异封装（unix/windows cfg）
- `runner::run()`：编排 + join + exit code 归一化（集成测试）

---

## 9. 与 stdin control channel 的协同注意事项

你前面要做“stdin 控制协议 JSONL”，这会引入 **第三条 I/O：child.stdin writer**。务必保证：

- child.stdin 只有一个 writer task（串行写 JSONL）
- tee 的 stdout/stderr pump 不与 stdin 写发生锁竞争（避免共享锁）
- 当你进入 Abort Sequence：
  - 先尝试写 `policy.abort`（若 stdin 仍可写）
  - 再发控制事件/kill
  - 最后 join stdout/stderr pump（读到 EOF）

---