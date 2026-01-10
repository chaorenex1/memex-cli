#!/usr/bin/env python3
"""
Project ID Utilities for Memex Hooks

提供从 cwd 参数自动生成 project_id 的功能。
零配置，完全自包含，无外部依赖。
"""

import os
import hashlib


def get_project_id_from_cwd(cwd: str) -> str:
    """
    从 cwd 生成 project_id

    格式: basename-hash8
    示例: memex_cli-a1b2c3d4

    Args:
        cwd: Hook 输入中的 cwd 字段（当前工作目录）

    Returns:
        project_id 字符串，格式为 "basename-hash8"

    Examples:
        >>> get_project_id_from_cwd("C:\\Users\\user\\projects\\memex_cli")
        'memex_cli-1a2b3c4d'

        >>> get_project_id_from_cwd("/home/user/projects/my-app")
        'my-app-5e6f7g8h'
    """
    if not cwd:
        return "default"

    # 获取目录名（basename）
    basename = os.path.basename(cwd)

    # 规范化路径（处理 Windows/Linux 差异）
    # 统一使用小写和正斜杠
    normalized = os.path.normpath(cwd).lower().replace('\\', '/')

    # 生成短哈希（8位足够）
    hash_obj = hashlib.sha256(normalized.encode('utf-8'))
    short_hash = hash_obj.hexdigest()[:8]

    # 组合：basename-hash
    project_id = f"{basename}-{short_hash}"

    # 清理非法字符
    return _sanitize_project_id(project_id)


def _sanitize_project_id(raw_id: str) -> str:
    """
    清理 project_id，确保符合规范

    规则：
    - 只保留字母、数字、连字符、下划线
    - 转换为小写
    - 移除连续的特殊字符
    - 限制长度为 64 字符

    Args:
        raw_id: 原始 project_id

    Returns:
        清理后的 project_id
    """
    if not raw_id:
        return "default"

    # 替换非字母数字字符为下划线（保留 - 和 _）
    sanitized = ''.join(
        c if c.isalnum() or c in '-_' else '_'
        for c in raw_id
    )

    # 转小写
    sanitized = sanitized.lower()

    # 移除连续的下划线和连字符
    while '__' in sanitized:
        sanitized = sanitized.replace('__', '_')
    while '--' in sanitized:
        sanitized = sanitized.replace('--', '-')

    # 去除首尾的下划线和连字符
    sanitized = sanitized.strip('_-')

    # 限制长度（最多 64 字符）
    if len(sanitized) > 64:
        sanitized = sanitized[:64]

    return sanitized if sanitized else "default"


# 示例用法和测试
if __name__ == "__main__":
    # 测试用例
    test_cases = [
        "C:\\Users\\zarag\\Documents\\aduib-app\\memex_cli",
        "/home/user/projects/my-app",
        "/var/www/html",
        "D:\\Code\\Projects\\Test Project",
        "",
        "project-with-special-chars!@#$%",
    ]

    print("=== Project ID Generation Test ===\n")
    for cwd in test_cases:
        project_id = get_project_id_from_cwd(cwd)
        print(f"CWD: {cwd or '(empty)'}")
        print(f"Project ID: {project_id}")
        print()
