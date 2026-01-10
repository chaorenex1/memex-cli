#!/usr/bin/env python3
"""
Long Text Handler - 长文本分块和处理模块
"""

import re
from dataclasses import dataclass
from typing import List, Tuple, Dict, Any, Optional


@dataclass
class TextChunk:
    """文本块"""
    index: int              # 块序号（从 0 开始）
    text: str               # 块内容
    start_pos: int          # 在原文中的起始位置
    end_pos: int            # 在原文中的结束位置
    is_truncated: bool      # 是否被截断
    metadata: Dict[str, Any]  # 额外元数据

    def __repr__(self):
        return (f"TextChunk(index={self.index}, "
                f"text='{self.text[:50]}...', "
                f"pos={self.start_pos}-{self.end_pos}, "
                f"truncated={self.is_truncated})")


class LongTextHandler:
    """长文本分块和处理"""

    # 配置常量
    MAX_CHUNK_SIZE = 8000           # 单个块最大字符数
    OVERLAP_SIZE = 200              # 块之间的重叠字符数
    MAX_CHUNKS = 20                 # 最大分块数量
    MAX_TOTAL_SIZE = 500000         # 最大总文本长度
    MAX_QUERY_SIZE = MAX_CHUNK_SIZE  # 查询最大长度
    MAX_ANSWER_SIZE = MAX_CHUNK_SIZE * 3  # 答案最大长度

    # 智能边界检测的正则
    SENTENCE_BOUNDARY = re.compile(r'[。！？\n\r]+|[.!?\n\r]+\s')
    PARAGRAPH_BOUNDARY = re.compile(r'\n\n+')

    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        初始化长文本处理器

        Args:
            config: 可选配置，可覆盖默认值
                {
                    "max_chunk_size": 8000,
                    "overlap_size": 200,
                    "max_chunks": 20,
                    "smart_boundary": True  # 是否使用智能边界检测
                }
        """
        if config:
            self.max_chunk_size = config.get("max_chunk_size", self.MAX_CHUNK_SIZE)
            self.overlap_size = config.get("overlap_size", self.OVERLAP_SIZE)
            self.max_chunks = config.get("max_chunks", self.MAX_CHUNKS)
            self.smart_boundary = config.get("smart_boundary", True)
        else:
            self.max_chunk_size = self.MAX_CHUNK_SIZE
            self.overlap_size = self.OVERLAP_SIZE
            self.max_chunks = self.MAX_CHUNKS
            self.smart_boundary = True

    def chunk_text(
        self,
        text: str,
        max_size: Optional[int] = None
    ) -> List[TextChunk]:
        """
        将长文本分块

        Args:
            text: 输入文本
            max_size: 单块最大大小（可选，默认使用 self.max_chunk_size）

        Returns:
            TextChunk 列表

        Examples:
            >>> handler = LongTextHandler()
            >>> chunks = handler.chunk_text("A" * 20000)
            >>> len(chunks)
            3
        """
        if not text:
            return []

        if max_size is None:
            max_size = self.max_chunk_size

        # 检查总长度
        if len(text) > self.MAX_TOTAL_SIZE:
            text = text[:self.MAX_TOTAL_SIZE]
            is_truncated_globally = True
        else:
            is_truncated_globally = False

        # 短文本直接返回
        if len(text) <= max_size:
            return [TextChunk(
                index=0,
                text=text,
                start_pos=0,
                end_pos=len(text),
                is_truncated=is_truncated_globally,
                metadata={}
            )]

        chunks = []
        current_pos = 0
        chunk_index = 0

        while current_pos < len(text) and chunk_index < self.max_chunks:
            # 计算块的结束位置
            chunk_end = min(current_pos + max_size, len(text))

            # 智能边界检测
            if self.smart_boundary and chunk_end < len(text):
                chunk_end = self._find_smart_boundary(text, current_pos, chunk_end)

            # 提取块文本
            chunk_text = text[current_pos:chunk_end]

            # 创建块
            is_last_chunk = chunk_end >= len(text)
            is_truncated = (chunk_index == self.max_chunks - 1 and not is_last_chunk) or is_truncated_globally

            chunk = TextChunk(
                index=chunk_index,
                text=chunk_text,
                start_pos=current_pos,
                end_pos=chunk_end,
                is_truncated=is_truncated,
                metadata={
                    "original_length": len(text),
                    "total_chunks": 0  # 将在最后更新
                }
            )
            chunks.append(chunk)

            # 下一个块的起始位置（带重叠）
            if not is_last_chunk:
                current_pos = chunk_end - self.overlap_size
                if current_pos <= chunks[-1].start_pos:
                    # 防止无限循环
                    current_pos = chunk_end
            else:
                break

            chunk_index += 1

        # 更新 total_chunks
        for chunk in chunks:
            chunk.metadata["total_chunks"] = len(chunks)

        return chunks

    def _find_smart_boundary(self, text: str, start: int, end: int) -> int:
        """
        在指定范围内查找智能边界（句子或段落结尾）

        Args:
            text: 完整文本
            start: 块起始位置
            end: 块建议结束位置

        Returns:
            调整后的结束位置
        """
        # 在 end 附近搜索范围（向前 200 字符）
        search_start = max(start, end - 200)
        search_text = text[search_start:end]

        # 优先查找段落边界
        para_matches = list(self.PARAGRAPH_BOUNDARY.finditer(search_text))
        if para_matches:
            last_para = para_matches[-1]
            return search_start + last_para.end()

        # 其次查找句子边界
        sent_matches = list(self.SENTENCE_BOUNDARY.finditer(search_text))
        if sent_matches:
            last_sent = sent_matches[-1]
            return search_start + last_sent.end()

        # 无智能边界，返回原始 end
        return end

    def merge_search_results(
        self,
        chunk_results: List[Dict[str, Any]],
        original_query: str
    ) -> Dict[str, Any]:
        """
        合并多个块的搜索结果

        Args:
            chunk_results: 各块的搜索结果列表
                [
                    {"matches": [{"qa_id": "qa-1", "score": 0.9, ...}, ...]},
                    {"matches": [{"qa_id": "qa-2", "score": 0.85, ...}, ...]},
                    ...
                ]
            original_query: 原始查询文本

        Returns:
            合并后的结果，按相关性排序
                {
                    "matches": [...],
                    "query": original_query,
                    "count": N,
                    "query_chunks": M,
                    "merged": True
                }
        """
        if not chunk_results:
            return {
                "matches": [],
                "query": original_query,
                "count": 0,
                "query_chunks": 0,
                "merged": True
            }

        # 收集所有匹配项
        all_matches = []
        for result in chunk_results:
            matches = result.get("matches", [])
            all_matches.extend(matches)

        if not all_matches:
            return {
                "matches": [],
                "query": original_query,
                "count": 0,
                "query_chunks": len(chunk_results),
                "merged": True
            }

        # 按 qa_id 去重，保留最高分数
        unique_matches = {}
        for match in all_matches:
            qa_id = match.get("qa_id")
            if not qa_id:
                continue

            score = match.get("score", 0.0)

            if qa_id not in unique_matches or score > unique_matches[qa_id].get("score", 0.0):
                unique_matches[qa_id] = match

        # 转换回列表并按分数排序
        merged_matches = sorted(
            unique_matches.values(),
            key=lambda m: m.get("score", 0.0),
            reverse=True
        )

        return {
            "matches": merged_matches,
            "query": original_query,
            "count": len(merged_matches),
            "query_chunks": len(chunk_results),
            "merged": True
        }

    def split_for_record(
        self,
        query: str,
        answer: str,
        max_query_size: Optional[int] = None,
        max_answer_size: Optional[int] = None
    ) -> List[Tuple[str, str, Dict[str, Any]]]:
        """
        为记录分割 Q&A 对

        策略：
        - 如果 query 和 answer 都不超长，返回单个 QA 对
        - 如果 answer 超长，分割 answer，每个块复用相同 query
        - 如果 query 也超长，截断 query（通常不应该发生）

        Args:
            query: 查询文本
            answer: 答案文本
            max_query_size: 查询最大长度（可选）
            max_answer_size: 答案最大长度（可选）

        Returns:
            [(query_chunk, answer_chunk, metadata), ...] 列表
            metadata 包含：
                {
                    "chunk_index": 0,
                    "total_chunks": 2,
                    "is_continuation": False  # 是否为续接块
                }
        """
        if max_query_size is None:
            max_query_size = self.MAX_QUERY_SIZE
        if max_answer_size is None:
            max_answer_size = self.MAX_ANSWER_SIZE

        # 处理 query 超长（截断）
        if len(query) > max_query_size:
            query = query[:max_query_size]

        # 答案不超长，直接返回
        if len(answer) <= max_answer_size:
            return [(query, answer, {
                "chunk_index": 0,
                "total_chunks": 1,
                "is_continuation": False
            })]

        # 分割答案
        answer_chunks = self.chunk_text(answer, max_size=max_answer_size)

        qa_pairs = []
        for chunk in answer_chunks:
            # 第一个块使用原始 query
            if chunk.index == 0:
                chunk_query = query
                is_continuation = False
            else:
                # 后续块使用"续接"标记
                chunk_query = f"{query} (续 {chunk.index + 1}/{len(answer_chunks)})"
                is_continuation = True

            metadata = {
                "chunk_index": chunk.index,
                "total_chunks": len(answer_chunks),
                "is_continuation": is_continuation,
                "answer_start_pos": chunk.start_pos,
                "answer_end_pos": chunk.end_pos
            }

            qa_pairs.append((chunk_query, chunk.text, metadata))

        return qa_pairs

    def validate_text_length(self, text: str, max_length: int, name: str = "text") -> Tuple[bool, Optional[str]]:
        """
        验证文本长度

        Args:
            text: 文本
            max_length: 最大长度
            name: 文本名称（用于错误消息）

        Returns:
            (is_valid, error_message)
        """
        if not text:
            return False, f"{name} is empty"

        if len(text) > self.MAX_TOTAL_SIZE:
            return False, f"{name} exceeds maximum total size ({self.MAX_TOTAL_SIZE} chars)"

        if len(text) > max_length:
            # 不算错误，只是警告
            return True, None

        return True, None


# 使用示例和测试
if __name__ == "__main__":
    # 基本测试
    handler = LongTextHandler()

    # 测试 1: 短文本
    short_text = "This is a short text."
    chunks = handler.chunk_text(short_text)
    print(f"Test 1 - Short text: {len(chunks)} chunk(s)")
    assert len(chunks) == 1
    assert not chunks[0].is_truncated

    # 测试 2: 长文本分块
    long_text = "A" * 20000
    chunks = handler.chunk_text(long_text, max_size=8000)
    print(f"Test 2 - Long text: {len(chunks)} chunk(s)")
    assert len(chunks) == 3
    assert chunks[0].text == "A" * 8000
    assert chunks[1].start_pos == 7800  # 重叠 200

    # 测试 3: 搜索结果合并
    chunk_results = [
        {"matches": [{"qa_id": "qa-1", "score": 0.9}, {"qa_id": "qa-2", "score": 0.7}]},
        {"matches": [{"qa_id": "qa-1", "score": 0.85}, {"qa_id": "qa-3", "score": 0.8}]}
    ]
    merged = handler.merge_search_results(chunk_results, "test query")
    print(f"Test 3 - Merge results: {len(merged['matches'])} unique match(es)")
    assert len(merged["matches"]) == 3  # 去重后
    assert merged["matches"][0]["qa_id"] == "qa-1"
    assert merged["matches"][0]["score"] == 0.9  # 取最高分

    # 测试 4: Q&A 分割
    query = "How to use Rust?"
    long_answer = "B" * 30000
    qa_pairs = handler.split_for_record(query, long_answer, max_answer_size=10000)
    print(f"Test 4 - QA split: {len(qa_pairs)} pair(s)")
    assert len(qa_pairs) > 1
    assert qa_pairs[0][2]["chunk_index"] == 0
    assert not qa_pairs[0][2]["is_continuation"]
    assert qa_pairs[1][2]["is_continuation"]

    print("\n✅ All tests passed!")
