#!/bin/bash
# Rust Gatekeeper 迁移验证脚本
# 用途: 验证配置是否正确加载

echo "=== Rust Gatekeeper 迁移验证 ==="
echo ""

# 1. 检查配置文件
echo "1. 检查 .claude/settings.json 配置..."
if grep -q "record-session-enhanced" .claude/settings.json; then
    echo "   ❌ 错误: settings.json 仍包含 record-session-enhanced hook"
    exit 1
else
    echo "   ✅ settings.json 已正确移除 Stop hook"
fi

if grep -q "memory-inject" .claude/settings.json; then
    echo "   ❌ 错误: settings.json 仍包含 memory-inject hook"
    exit 1
else
    echo "   ✅ settings.json 已正确移除 UserPromptSubmit hook"
fi

# 2. 检查文件是否已归档
echo ""
echo "2. 检查文件归档状态..."
if [ -f ".claude/hooks/record-session-enhanced.py" ]; then
    echo "   ⚠️  警告: record-session-enhanced.py 仍在 hooks 目录"
    echo "   建议: 移动到归档目录"
else
    echo "   ✅ record-session-enhanced.py 已移除"
fi

if [ -f ".claude/hooks/memory-inject.py" ]; then
    echo "   ⚠️  警告: memory-inject.py 仍在 hooks 目录"
    echo "   建议: 移动到归档目录"
else
    echo "   ✅ memory-inject.py 已移除"
fi

if [ -f ".claude/hooks/gatekeeper.py" ]; then
    echo "   ⚠️  警告: gatekeeper.py 仍在 hooks 目录"
else
    echo "   ✅ gatekeeper.py 已移除"
fi

# 3. 检查归档目录
echo ""
echo "3. 检查归档目录..."
if [ -d ".claude/hooks/_archived_2026-01-11_rust-migration" ]; then
    archived_count=$(ls .claude/hooks/_archived_2026-01-11_rust-migration/*.py 2>/dev/null | wc -l)
    echo "   ✅ 归档目录存在，包含 $archived_count 个文件"
else
    echo "   ⚠️  归档目录不存在"
fi

# 4. 检查 Python 字节码缓存
echo ""
echo "4. 检查 Python 字节码缓存..."
if [ -f ".claude/hooks/__pycache__/record-session-enhanced.cpython-312.pyc" ]; then
    echo "   ⚠️  警告: 发现字节码缓存，正在清理..."
    rm -f .claude/hooks/__pycache__/record-session-enhanced.* \
          .claude/hooks/__pycache__/memory-inject.* \
          .claude/hooks/__pycache__/gatekeeper.*
    echo "   ✅ 缓存已清理"
else
    echo "   ✅ 无字节码缓存"
fi

# 5. 检查 config.toml
echo ""
echo "5. 检查 Rust gatekeeper 配置..."
if grep -q 'provider = "standard"' config.toml; then
    echo "   ✅ config.toml 已配置 Rust StandardGatekeeper"
else
    echo "   ⚠️  警告: config.toml 未找到 gatekeeper provider"
fi

echo ""
echo "=== 验证完成 ==="
echo ""
echo "📋 下一步操作:"
echo "1. 完全退出 Claude Code（确保所有进程终止）"
echo "2. 重新启动 Claude Code"
echo "3. 执行测试任务，观察是否仍有 Stop hook 错误"
echo ""
echo "如果重启后仍有错误，请执行:"
echo "  cat .claude/settings.json"
echo "  并检查 Claude Code 实际加载的配置"
