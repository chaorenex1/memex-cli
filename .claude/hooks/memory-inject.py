#!/usr/bin/env python3
"""
Memory Inject Hook for Claude Code
Triggers on: UserPromptSubmit
Purpose: Search memory service and inject relevant context (HTTP Server Version)
"""

import sys
import json
import subprocess
import os
from pathlib import Path
from datetime import datetime
from project_utils import get_project_id_from_cwd
from session_state import update_session_state
from http_client import HTTPClient, direct_cli_call


def log_debug(message):
    """Log debug message to file"""
    hook_dir = Path.home().joinpath(".memex", "logs")
    log_file = hook_dir.joinpath("memory-inject.log")
    try:
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(log_file, "a", encoding="utf-8") as f:
            f.write(f"{timestamp} {message}\n")
    except:
        pass


def search_memory_with_fallback(
    session_id: str,
    query: str,
    project_id: str,
    limit: int = 5,
    min_score: float = 0.6
):
    """
    搜索记忆，优先使用HTTP服务器，失败时降级到直接调用

    Args:
        session_id: 会话 ID
        query: 搜索查询
        project_id: 项目 ID
        limit: 最大结果数
        min_score: 最低相关性分数

    Returns:
        搜索结果字典，如果失败返回 None
    """
    # 方案 A: 尝试使用HTTP服务器
    try:
        log_debug("Attempting to use HTTP server for search...")
        client = HTTPClient(session_id)

        response = client.search(
            query=query,
            project_id=project_id,
            limit=limit,
            min_score=min_score
        )

        if response.get("success"):
            log_debug("✓ Search via HTTP server succeeded")
            return response.get("data", {})
        else:
            error = response.get("error", "Unknown error")
            log_debug(f"HTTP server returned error: {error}, falling back to direct call")
            # 继续尝试直接调用

    except Exception as e:
        log_debug(f"HTTP server unavailable: {e}, falling back to direct call")
        # 继续尝试直接调用

    # 方案 B: 降级到直接调用 memex-cli
    try:
        log_debug("Using direct memex-cli call...")
        result = direct_cli_call("search", {
            "project-id": project_id,
            "query": query,
            "limit": limit,
            "min-score": min_score
        })

        if result.get("success"):
            log_debug("✓ Direct call succeeded")
            return result.get("data")
        else:
            log_debug(f"Direct call failed: {result.get('error')}")
            return None

    except Exception as e:
        log_debug(f"Direct call error: {e}")
        return None


def main():
    try:
        # 读取 Hook 输入
        hook_input = json.loads(sys.stdin.read())
        log_debug(f"Hook triggered: {json.dumps(hook_input, ensure_ascii=False)[:200]}")

        user_prompt = hook_input.get("prompt", "")
        cwd = hook_input.get("cwd", os.getcwd())
        session_id = hook_input.get("session_id", "unknown")

        # 跳过条件
        if not user_prompt.strip() or len(user_prompt.strip()) < 10:
            log_debug("Skipping: prompt too short or empty")
            sys.exit(0)

        # 生成 project_id
        project_id = get_project_id_from_cwd(cwd)
        log_debug(f"Project ID: {project_id}")

        # 搜索记忆（优先使用守护进程，失败时降级到直接调用）
        search_result = search_memory_with_fallback(
            session_id=session_id,
            query=user_prompt,
            project_id=project_id,
            limit=5,
            min_score=0.6
        )

        if search_result is None:
            log_debug("Search failed with both daemon and direct call")
            sys.exit(0)

        matches = search_result.get("matches", [])

        if not matches:
            log_debug("No matches found")
            sys.exit(0)

        # 提取 shown_qa_ids
        shown_qa_ids = [m.get("qa_id", "") for m in matches if m.get("qa_id")]

        # 格式化为 Markdown 上下文（使用 HTML 注释标记 QA ID）
        context_lines = [
            "### 📚 相关历史记忆\n",
            "以下是从记忆系统中检索到的相关知识，优先使用相关性高的内容。\n",
            "**重要**：如果你使用了某条知识，必须在回答中保留其 HTML 注释标记（`<!-- memex-qa:ID -->`），以便追踪知识使用情况。\n",
            "**使用规则**：",
            "- 优先使用相关性评分高的知识",
            "- 如果知识不相关，可以忽略",
            "- 使用知识时保持其 HTML 注释标记不变",
            "- 不要编造不存在的知识\n"
        ]

        for match in matches:
            qa_id = match.get("qa_id", "unknown")
            question = match.get("question", "")
            answer = match.get("answer", "")
            score = match.get("score", 0.0)

            # 使用 HTML 注释标记（不可见）
            context_lines.append(f"<!-- memex-qa:{qa_id} -->")
            context_lines.append(f"**Q**: {question}")
            context_lines.append(f"**A**: {answer}")
            context_lines.append(f"_相关性: {score:.2f}_")
            context_lines.append(f"<!-- /memex-qa -->\n---\n")

        additional_context = "\n".join(context_lines)
        log_debug(f"Injecting {len(matches)} matches with QA IDs: {shown_qa_ids}")

        # 保存到会话状态（供 Stop Hook 使用）
        update_session_state(session_id, {
            "shown_qa_ids": shown_qa_ids,
            "query": user_prompt
        })
        log_debug(f"Saved shown_qa_ids to session state")

        # 输出 Hook 响应
        response = {
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": additional_context
            },
            "continue": True,
            "suppressOutput": False
        }

        print(json.dumps(response, ensure_ascii=False))
        log_debug("Memory inject completed successfully")
        sys.exit(0)

    except subprocess.TimeoutExpired:
        log_debug("Search timeout")
        sys.exit(0)
    except Exception as e:
        log_debug(f"Unexpected error: {e}")
        import traceback
        log_debug(traceback.format_exc())
        sys.exit(0)


if __name__ == "__main__":
    main()
