#!/usr/bin/env python3
"""
Memory Record Hook for Claude Code
Triggers on: PostToolUse
Purpose: Record tool usage as memory candidates (HTTP Server Version)
"""

import sys
import json
import subprocess
import os
import threading
from pathlib import Path
from datetime import datetime
from project_utils import get_project_id_from_cwd
from http_client import HTTPClient, direct_cli_call

# 跳过的文件类型
SKIP_EXTENSIONS = ['.json', '.toml', '.yml', '.yaml', '.xml', '.ini', '.env']
SKIP_PATHS = ['.git/', 'node_modules/', '__pycache__/', '.venv/', 'venv/']


def log_debug(message):
    """Log debug message to file"""
    hook_dir = Path.home().joinpath(".memex", "logs")
    log_file = hook_dir.joinpath("memory-record.log")
    try:
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(log_file, "a", encoding="utf-8") as f:
            f.write(f"{timestamp} {message}\n")
    except:
        pass


def should_record_file(file_path):
    """Check if file should be recorded"""
    if not file_path:
        return False
    for ext in SKIP_EXTENSIONS:
        if file_path.endswith(ext):
            return False
    for skip_path in SKIP_PATHS:
        if skip_path in file_path:
            return False
    return True


def extract_language_tag(file_path):
    """Extract language tag from file extension"""
    ext_map = {
        '.py': 'python', '.js': 'javascript', '.ts': 'typescript',
        '.tsx': 'typescript-react', '.jsx': 'javascript-react',
        '.rs': 'rust', '.go': 'golang', '.java': 'java',
        '.cpp': 'cpp', '.c': 'c', '.sh': 'bash',
        '.md': 'markdown', '.txt': 'text'
    }
    for ext, lang in ext_map.items():
        if file_path.endswith(ext):
            return lang
    return "unknown"


def handle_generic_tool(tool_name, tool_input, tool_response):
    """
    通用工具处理器 - 支持所有未特殊处理的工具

    Returns:
        tuple: (query, answer, tags) 或 None
    """
    # 提取输入参数（限制长度避免过大）
    input_str = json.dumps(tool_input, ensure_ascii=False, indent=2)
    if len(input_str) > 500:
        input_str = input_str[:500] + "\n... (truncated)"

    # 提取输出响应
    response_content = ""
    is_error = False
    if tool_response:
        response_content = tool_response.get("content", "")
        is_error = tool_response.get("is_error", False)

    # 限制响应长度
    if len(response_content) > 500:
        response_content = response_content[:500] + "\n... (truncated)"

    # 构建查询和答案
    query = f"使用工具: {tool_name}"

    if response_content:
        answer = f"**工具**: {tool_name}\n\n**输入**:\n```json\n{input_str}\n```\n\n**输出**:\n```\n{response_content}\n```"
    else:
        answer = f"**工具**: {tool_name}\n\n**输入**:\n```json\n{input_str}\n```\n\n**输出**: (无)"

    # 构建标签
    tags = [f"tool:{tool_name.lower()}", "auto-record"]
    if is_error:
        tags.append("tool-error")
    else:
        tags.append("tool-success")

    return query, answer, tags


def record_via_http_async(session_id, query, answer, tags_str, metadata, project_id, log_func):
    """
    通过HTTP服务器异步记录（在后台线程中）
    """
    def _record():
        try:
            client = HTTPClient(session_id)

            response = client.record_candidate(
                project_id=project_id,
                question=query,
                answer=answer
            )

            if response.get("success"):
                log_func("✓ Async recording via HTTP server succeeded")
            else:
                log_func(f"✗ HTTP server recording failed: {response.get('error', 'Unknown')}")
        except Exception as e:
            log_func(f"✗ HTTP server recording error: {e}")

    # 在后台线程中执行
    threading.Thread(target=_record, daemon=True).start()
    return True


def record_async(cmd, log_func):
    """
    异步非阻塞记录 - 提升并发性能

    使用 Popen 启动独立进程，不等待完成
    """
    try:
        # 跨平台异步启动
        if os.name == 'nt':  # Windows
            # Windows: 使用 DETACHED_PROCESS 标志
            subprocess.Popen(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                stdin=subprocess.DEVNULL,
                creationflags=subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
            )
        else:  # Unix/Linux/macOS
            # Unix: 使用 start_new_session
            subprocess.Popen(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                stdin=subprocess.DEVNULL,
                start_new_session=True
            )
        log_func(f"✓ Async recording started")
        return True
    except Exception as e:
        log_func(f"✗ Failed to start async recording: {e}")
        return False


def record_with_fallback(session_id, project_id, query, answer, tags_str, metadata, log_func):
    """
    记录候选知识，优先使用HTTP服务器，失败时降级到直接调用
    """
    # 方案 A: 尝试使用HTTP服务器（异步）
    try:
        # HTTP服务器可用，使用异步方式
        record_via_http_async(session_id, query, answer, tags_str, metadata, project_id, log_func)
        return True
    except Exception as e:
        log_func(f"HTTP server unavailable: {e}, falling back to direct call")

        # 方案 B: 降级到直接调用
        cmd = [
            "memex-cli", 'record-candidate',
            '--project-id', project_id,
            '--query', query,
            '--answer', answer,
            '--tags', tags_str,
            '--metadata', metadata
        ]
        return record_async(cmd, log_func)


def main():
    try:
        # Read Hook input from stdin
        hook_input = json.loads(sys.stdin.read())
        log_debug(f"Hook triggered: tool={hook_input.get('tool_name', 'unknown')}")

        tool_name = hook_input.get("tool_name", "")
        tool_input = hook_input.get("tool_input", {})
        tool_response = hook_input.get("tool_response", {})
        cwd = hook_input.get("cwd", os.getcwd())
        session_id = hook_input.get("session_id", "unknown")

        # 生成 project_id
        project_id = get_project_id_from_cwd(cwd)
        log_debug(f"Project ID: {project_id}, Tool: {tool_name}")

        memex_cli = "memex-cli"
        query = ""
        answer = ""
        tags = []

        # 处理 Write 工具
        if tool_name == "Write":
            file_path = tool_input.get('file_path', '')
            content = tool_input.get('content', '')

            if not should_record_file(file_path):
                log_debug(f"Skipping Write: {file_path} (filtered)")
                sys.exit(0)

            if len(content) < 100:
                log_debug(f"Skipping Write: {file_path} (content too short)")
                sys.exit(0)

            query = f"创建文件 {file_path}"
            content_snippet = content[:300] if len(content) > 300 else content
            answer = f"文件内容片段：\n```\n{content_snippet}\n```"

            lang = extract_language_tag(file_path)
            tags.extend([f"lang:{lang}", "file-creation"])

        # 处理 Edit 工具
        elif tool_name == "Edit":
            file_path = tool_input.get('file_path', '')
            old_string = tool_input.get('old_string', '')
            new_string = tool_input.get('new_string', '')

            if not should_record_file(file_path):
                log_debug(f"Skipping Edit: {file_path} (filtered)")
                sys.exit(0)

            query = f"编辑文件 {file_path}"
            old_snippet = old_string[:100] if len(old_string) > 100 else old_string
            new_snippet = new_string[:100] if len(new_string) > 100 else new_string
            answer = f"代码修改：\n从：\n```\n{old_snippet}\n```\n改为：\n```\n{new_snippet}\n```"

            lang = extract_language_tag(file_path)
            tags.extend([f"lang:{lang}", "file-edit"])
        
        elif tool_name == "Read":
            file_path = tool_input.get('file_path', '')
            content = tool_response.get('content', '')

            if not should_record_file(file_path):
                log_debug(f"Skipping Read: {file_path} (filtered)")
                sys.exit(0)

            if len(content) < 100:
                log_debug(f"Skipping Read: {file_path} (content too short)")
                sys.exit(0)

            query = f"读取文件 {file_path}"
            content_snippet = content[:300] if len(content) > 300 else content
            answer = f"文件内容片段：\n```\n{content_snippet}\n```"

            lang = extract_language_tag(file_path)
            tags.extend([f"lang:{lang}", "file-read"])

        # 处理 Bash 工具
        elif tool_name == "Bash":
            command = tool_input.get('command', '')

            # 跳过简单命令
            trivial_commands = ['ls', 'pwd', 'cd', 'dir', 'echo', 'cat']
            if not command or len(command) < 10 or command.strip().split()[0] in trivial_commands:
                log_debug(f"Skipping Bash: '{command}' (trivial)")
                sys.exit(0)

            query = f"执行命令: {command}"

            result_content = tool_response.get('content', '') if tool_response else ''
            is_error = tool_response.get('is_error', False) if tool_response else False

            if is_error:
                answer = f"命令失败：`{command}`\n错误：\n```\n{result_content[:200]}\n```"
                tags.append("command-error")
            else:
                result_snippet = result_content[:200] if result_content else "(无输出)"
                answer = f"成功执行：`{command}`\n输出：\n```\n{result_snippet}\n```"
                tags.append("command-success")

        # 处理 Skill 工具
        elif tool_name == "Skill":
            skill_name = tool_input.get("skill", "")
            skill_args = tool_input.get("args", "")

            if not skill_name:
                log_debug("Skipping Skill: no skill name")
                sys.exit(0)

            query = f"调用技能: {skill_name}"

            response_content = tool_response.get("content", "") if tool_response else ""
            is_error = tool_response.get("is_error", False) if tool_response else False

            if is_error:
                answer = f"技能执行失败：`{skill_name}`\n参数：{skill_args}\n错误：\n```\n{response_content[:200]}\n```"
                tags.append("skill-error")
            else:
                content_snippet = response_content[:300] if response_content else "(无输出)"
                answer = f"技能执行：`{skill_name}`\n参数：{skill_args}\n输出：\n```\n{content_snippet}\n```"
                tags.append("skill-success")

            tags.append(f"skill:{skill_name}")

        # 处理其他所有工具（通用处理器）
        # else:
        #     result = handle_generic_tool(tool_name, tool_input, tool_response)
        #     if result:
        #         query, answer, tags = result
        #         log_debug(f"Generic tool handled: {tool_name}")
        #     else:
        #         log_debug(f"Skipping: generic handler returned None for {tool_name}")
        #         sys.exit(0)

        # 记录候选
        if query and answer:
            tags_str = ','.join(tags)
            metadata = json.dumps({"session_id": session_id})

            log_debug(f"Recording: {query[:50]}...")

            # 异步记录（优先使用守护进程，失败时降级到直接调用）
            record_with_fallback(
                session_id=session_id,
                project_id=project_id,
                query=query,
                answer=answer,
                tags_str=tags_str,
                metadata=metadata,
                log_func=log_debug
            )

        sys.exit(0)
    except Exception as e:
        log_debug(f"Unexpected error: {e}")
        sys.exit(0)

if __name__ == "__main__":
    main()
