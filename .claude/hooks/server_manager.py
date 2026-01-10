#!/usr/bin/env python3
"""
HTTP服务器管理器 - 管理 Rust memex-cli HTTP 服务器生命周期
"""
import os
import sys
import time
import subprocess
import json
import signal
from pathlib import Path
from typing import Optional, Dict


class ServerManager:
    """HTTP服务器管理器（Rust memex-cli）"""

    def __init__(self, session_id: str):
        self.session_id = session_id
        self.servers_dir = Path.home() / ".memex" / "servers"
        self.servers_dir.mkdir(parents=True, exist_ok=True)

        self.state_file = self.servers_dir / f"memex-{session_id}.state"
        self.log_file = self.servers_dir / f"memex-{session_id}.log"

    def load_state(self) -> Optional[Dict]:
        """加载服务器状态"""
        if not self.state_file.exists():
            return None

        try:
            with open(self.state_file, 'r', encoding='utf-8') as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            return None

    @staticmethod
    def _is_process_alive(pid: int) -> bool:
        """检查进程是否存活"""
        try:
            if os.name == 'nt':  # Windows
                result = subprocess.run(
                    ["tasklist", "/FI", f"PID eq {pid}"],
                    capture_output=True,
                    text=True,
                    timeout=2
                )
                return str(pid) in result.stdout
            else:  # Unix
                os.kill(pid, 0)
                return True
        except (OSError, subprocess.TimeoutExpired):
            return False

    def is_server_running(self) -> bool:
        """检查服务器是否运行"""
        state = self.load_state()

        if not state:
            return False

        pid = state.get('pid')
        if not pid:
            return False

        return self._is_process_alive(pid)

    def start_server(self) -> bool:
        """
        启动 Rust HTTP 服务器

        Returns:
            是否启动成功
        """
        # 检查服务器是否已运行
        if self.is_server_running():
            return True

        # 构建启动命令
        command = [
            "memex-cli",
            "http-server",
            "--session-id", self.session_id
        ]

        # 启动服务器
        try:
            log_fd = open(self.log_file, "a", encoding="utf-8")

            # 跨平台启动
            if os.name == 'nt':  # Windows
                subprocess.Popen(
                    command,
                    stdout=log_fd,
                    stderr=log_fd,
                    stdin=subprocess.DEVNULL,
                    creationflags=subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP,
                    close_fds=False
                )
            else:  # Unix
                subprocess.Popen(
                    command,
                    stdout=log_fd,
                    stderr=log_fd,
                    stdin=subprocess.DEVNULL,
                    start_new_session=True,
                    close_fds=True
                )

            log_fd.close()

            # 等待服务器写入状态文件
            time.sleep(1)

            # 验证服务器运行
            return self.is_server_running()

        except Exception as e:
            print(f"Failed to start server: {e}", file=sys.stderr)
            return False

    def stop_server(self, timeout: int = 10) -> bool:
        """
        停止HTTP服务器

        Args:
            timeout: 超时时间（秒）

        Returns:
            是否停止成功
        """
        state = self.load_state()

        if not state:
            return True

        pid = state.get('pid')
        if not pid:
            return True

        if not self._is_process_alive(pid):
            self._cleanup()
            return True

        try:
            # 优雅关闭
            if os.name == 'nt':  # Windows
                subprocess.run(
                    ["taskkill", "/F", "/PID", str(pid)],
                    capture_output=True,
                    timeout=timeout
                )
            else:  # Unix
                os.kill(pid, signal.SIGTERM)

                # 等待退出
                for _ in range(timeout * 10):
                    if not self._is_process_alive(pid):
                        break
                    time.sleep(0.1)

                # 强制kill
                if self._is_process_alive(pid):
                    os.kill(pid, signal.SIGKILL)
                    time.sleep(0.5)

            # 清理
            self._cleanup()
            return True

        except Exception as e:
            print(f"Failed to stop server: {e}", file=sys.stderr)
            return False

    def get_server_url(self) -> Optional[str]:
        """获取服务器URL"""
        state = self.load_state()
        return state.get('url') if state else None

    def get_server_port(self) -> Optional[int]:
        """获取服务器端口"""
        state = self.load_state()
        return state.get('port') if state else None

    def _cleanup(self):
        """清理服务器状态文件"""
        if self.state_file.exists():
            self.state_file.unlink()
