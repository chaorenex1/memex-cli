#!/usr/bin/env python3
"""
Session State Management for Memex Hooks

管理 Hook 之间共享的会话状态，通过文件系统存储。
主要用于在 UserPromptSubmit 和 Stop Hook 之间传递 shown_qa_ids。
"""

import json
import os
from pathlib import Path
from typing import Dict, Any, Optional
from pathlib import Path


# 会话状态存储目录
STATE_DIR = Path.home().joinpath(".memex", "session_states")


def ensure_state_dir():
    """确保状态目录存在"""
    STATE_DIR.mkdir(parents=True, exist_ok=True)


def get_state_file_path(session_id: str) -> str:
    """
    获取会话状态文件路径

    Args:
        session_id: 会话 ID

    Returns:
        状态文件的完整路径
    """
    ensure_state_dir()
    return os.path.join(STATE_DIR, f"{session_id}.json")


def save_session_state(session_id: str, state: Dict[str, Any]) -> None:
    """
    保存会话状态

    Args:
        session_id: 会话 ID
        state: 要保存的状态字典

    Example:
        >>> save_session_state("session-123", {
        ...     "shown_qa_ids": ["qa-abc", "qa-def"],
        ...     "project_id": "memex_cli-1a2b3c4d",
        ...     "query": "如何优化性能？"
        ... })
    """
    if not session_id:
        return

    state_file = get_state_file_path(session_id)

    try:
        with open(state_file, 'w', encoding='utf-8') as f:
            json.dump(state, f, ensure_ascii=False, indent=2)
    except Exception as e:
        # 静默失败，不影响 Hook 执行
        pass


def load_session_state(session_id: str) -> Dict[str, Any]:
    """
    加载会话状态

    Args:
        session_id: 会话 ID

    Returns:
        状态字典，如果不存在则返回空字典

    Example:
        >>> state = load_session_state("session-123")
        >>> shown_qa_ids = state.get("shown_qa_ids", [])
    """
    if not session_id:
        return {}

    state_file = get_state_file_path(session_id)

    if not os.path.exists(state_file):
        return {}

    try:
        with open(state_file, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception as e:
        # 解析失败返回空字典
        return {}


def update_session_state(session_id: str, updates: Dict[str, Any]) -> None:
    """
    更新会话状态（合并更新）

    Args:
        session_id: 会话 ID
        updates: 要更新的字段

    Example:
        >>> update_session_state("session-123", {
        ...     "shown_qa_ids": ["qa-abc", "qa-def"]
        ... })
    """
    if not session_id:
        return

    # 加载现有状态
    current_state = load_session_state(session_id)

    # 合并更新
    current_state.update(updates)

    # 保存
    save_session_state(session_id, current_state)


def clear_session_state(session_id: str) -> None:
    """
    清理会话状态文件

    Args:
        session_id: 会话 ID

    Example:
        >>> clear_session_state("session-123")
    """
    if not session_id:
        return

    state_file = get_state_file_path(session_id)

    if os.path.exists(state_file):
        try:
            os.remove(state_file)
        except Exception:
            # 静默失败
            pass


def cleanup_old_states(max_age_hours: int = 24) -> int:
    """
    清理旧的会话状态文件

    Args:
        max_age_hours: 最大保留时间（小时）

    Returns:
        清理的文件数量
    """
    if not os.path.exists(STATE_DIR):
        return 0

    import time
    current_time = time.time()
    max_age_seconds = max_age_hours * 3600
    cleaned_count = 0

    try:
        for filename in os.listdir(STATE_DIR):
            if not filename.endswith('.json'):
                continue

            file_path = os.path.join(STATE_DIR, filename)
            file_age = current_time - os.path.getmtime(file_path)

            if file_age > max_age_seconds:
                try:
                    os.remove(file_path)
                    cleaned_count += 1
                except Exception:
                    pass
    except Exception:
        pass

    return cleaned_count


# 示例用法
if __name__ == "__main__":
    # 测试会话状态管理
    test_session_id = "test-session-123"

    print("=== Session State Test ===\n")

    # 保存状态
    print("1. Saving session state...")
    save_session_state(test_session_id, {
        "shown_qa_ids": ["qa-abc123", "qa-def456"],
        "project_id": "memex_cli-1a2b3c4d",
        "query": "如何优化 Rust 编译速度？",
        "timestamp": "2026-01-08 10:00:00"
    })
    print("   ✓ Saved")

    # 加载状态
    print("\n2. Loading session state...")
    state = load_session_state(test_session_id)
    print(f"   Loaded: {json.dumps(state, ensure_ascii=False, indent=2)}")

    # 更新状态
    print("\n3. Updating session state...")
    update_session_state(test_session_id, {
        "used_qa_ids": ["qa-abc123"]
    })
    print("   ✓ Updated")

    # 再次加载
    print("\n4. Loading updated state...")
    state = load_session_state(test_session_id)
    print(f"   Loaded: {json.dumps(state, ensure_ascii=False, indent=2)}")

    # 清理
    print("\n5. Cleaning up...")
    clear_session_state(test_session_id)
    print("   ✓ Cleaned")

    # 验证清理
    print("\n6. Verifying cleanup...")
    state = load_session_state(test_session_id)
    print(f"   State after cleanup: {state}")

    print("\n=== Test Complete ===")
