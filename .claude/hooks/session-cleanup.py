#!/usr/bin/env python3
"""
Session Cleanup Hook for Claude Code
Triggers on: SessionEnd
Purpose: Clean up session state files (HTTP Server Version)
"""

import sys
import json
import os
import pathlib
from datetime import datetime
from project_utils import get_project_id_from_cwd
from session_state import clear_session_state, cleanup_old_states
from server_manager import ServerManager


def log_debug(message):
    """Log debug message to file"""
    hook_dir = pathlib.Path.home().joinpath(".memex", "logs")
    log_file = hook_dir.joinpath("session-cleanup.log")
    try:
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(log_file, "a", encoding="utf-8") as f:
            f.write(f"{timestamp} {message}\n")
    except:
        pass


def main():
    try:
        # 读取 Hook 输入
        hook_input = json.loads(sys.stdin.read())

        session_id = hook_input.get("session_id", "unknown")
        reason = hook_input.get("reason", "unknown")
        cwd = hook_input.get("cwd", os.getcwd())

        log_debug(f"=== Session Cleanup ===")
        log_debug(f"Session: {session_id}")
        log_debug(f"Reason: {reason}")

        # 生成 project_id（用于日志）
        project_id = get_project_id_from_cwd(cwd)
        log_debug(f"Project ID: {project_id}")

        # 停止HTTP服务器
        try:
            server_manager = ServerManager(session_id)
            if server_manager.is_server_running():
                log_debug("Stopping memex HTTP server...")
                if server_manager.stop_server():
                    log_debug("✓ Memex HTTP server stopped successfully")
                else:
                    log_debug("✗ Failed to stop memex HTTP server")
            else:
                log_debug("Memex HTTP server not running")
        except Exception as e:
            log_debug(f"Warning: Could not stop HTTP server: {e}")

        # 清理当前会话状态
        clear_session_state(session_id)
        log_debug("Session state cleared")

        # 清理旧的会话状态文件（超过 24 小时）
        cleaned_count = cleanup_old_states(max_age_hours=24)
        if cleaned_count > 0:
            log_debug(f"Cleaned up {cleaned_count} old session state files")

        log_debug("=== Session Cleanup Complete ===")
        sys.exit(0)

    except Exception as e:
        log_debug(f"Error in session-cleanup: {e}")
        import traceback
        log_debug(traceback.format_exc())
        sys.exit(0)


if __name__ == "__main__":
    main()
