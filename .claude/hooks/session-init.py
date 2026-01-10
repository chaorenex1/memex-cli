#!/usr/bin/env python3
"""
Session Init Hook (HTTP Server Version)
零阻塞版本：使用后台线程启动HTTP服务器
"""

import sys
import json
import os
import threading
from pathlib import Path
from datetime import datetime
from project_utils import get_project_id_from_cwd
from session_state import save_session_state, cleanup_old_states
from server_manager import ServerManager


def log_debug(message):
    """Log debug message to file"""
    hook_dir = Path.home().joinpath(".memex", "logs")
    log_file = hook_dir.joinpath("session-init.log")
    try:
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(log_file, "a", encoding="utf-8") as f:
            f.write(f"[{timestamp}] {message}\n")
    except:
        pass


def start_server_async(session_id):
    """后台线程：启动 Rust HTTP 服务器（零阻塞）"""
    try:
        log_debug(f"[Thread] Starting Rust HTTP server for session {session_id}")
        server_manager = ServerManager(session_id)

        # 检查服务器是否已运行
        if server_manager.is_server_running():
            log_debug("[Thread] Server already running, skipping start")
            return

        # 启动 Rust HTTP 服务器
        success = server_manager.start_server()

        if success:
            port = server_manager.get_server_port()
            url = server_manager.get_server_url()
            log_debug(f"[Thread] ✓ Rust HTTP server started: {url} (port {port})")
        else:
            log_debug("[Thread] ✗ Failed to start Rust HTTP server")
            # 检查日志文件
            log_file = server_manager.log_file
            if log_file.exists() and log_file.stat().st_size > 0:
                log_debug(f"[Thread] Check server log: {log_file}")
            else:
                log_debug(f"[Thread] Server log is empty or missing: {log_file}")

    except Exception as e:
        log_debug(f"[Thread] ERROR: Server thread exception: {e}")
        import traceback
        log_debug(f"[Thread] Traceback:\n{traceback.format_exc()}")


def main():
    try:
        # 读取 Hook 输入
        hook_input = json.loads(sys.stdin.read())

        cwd = hook_input.get("cwd", os.getcwd())
        session_id = hook_input.get("session_id", "unknown")
        source = hook_input.get("source", "unknown")

        log_debug(f"=== Session Init (HTTP Server) ===")
        log_debug(f"Session ID: {session_id}")
        log_debug(f"Source: {source}")
        log_debug(f"CWD: {cwd}")

        # 生成 project_id
        project_id = get_project_id_from_cwd(cwd)
        log_debug(f"Project ID: {project_id}")

        # 初始化会话状态
        save_session_state(session_id, {
            "project_id": project_id,
            "cwd": cwd,
            "start_time": datetime.now().isoformat(),
            "source": source
        })
        log_debug("Session state initialized")

        # 清理旧状态
        cleaned_count = cleanup_old_states(max_age_hours=24)
        if cleaned_count > 0:
            log_debug(f"Cleaned up {cleaned_count} old session state files")

        # 🚀 零阻塞：在后台线程中启动HTTP服务器
        server_thread = threading.Thread(
            target=start_server_async,
            args=(session_id,),
            daemon=False  # 非守护线程，确保服务器启动完成
        )
        server_thread.start()
        log_debug("HTTP server thread launched (non-blocking)")

        # 等待服务器启动完成（最多2秒）
        server_thread.join(timeout=2.0)
        if server_thread.is_alive():
            log_debug("Server thread still running, continuing without wait")

        # 返回上下文
        response = {
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": f"### 项目: {project_id}\n会话: {session_id}"
            }
        }

        try:
            output = json.dumps(response, ensure_ascii=True)
            print(output)
        except UnicodeEncodeError:
            output = json.dumps(response, ensure_ascii=True)
            sys.stdout.buffer.write(output.encode('utf-8'))
            sys.stdout.buffer.write(b'\n')
            sys.stdout.buffer.flush()

        log_debug("Session init completed (zero-blocking mode)")
        sys.exit(0)

    except Exception as e:
        log_debug(f"Error in session-init: {e}")
        import traceback
        log_debug(traceback.format_exc())
        sys.exit(0)


if __name__ == "__main__":
    main()
