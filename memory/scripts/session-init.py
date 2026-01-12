#!/usr/bin/env python3
"""
Session Init Hook (HTTP Server Version)
零阻塞版本：启动服务器后立即返回
"""

import sys
import json
import os
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

        # 🚀 零阻塞：启动 Rust HTTP 服务器（不等待就绪）
        server_manager = ServerManager(session_id)
        started = server_manager.start_server(wait_for_ready=False)
        log_debug(f"Rust HTTP server launch requested (started={started})")

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
