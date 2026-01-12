#!/usr/bin/env python3
"""
Claude Code Transcript Parser
Parses Claude Code JSONL transcript files and extracts structured data
"""

import json
import re
import sys
from typing import Dict, List, Optional, Any
from pathlib import Path
from dataclasses import dataclass, asdict

from session_state import load_session_state

@dataclass
class ToolEventLite:
    """Lightweight tool event representation"""
    tool: str
    args: Dict[str, Any]
    output: Optional[str] = None
    code: Optional[int] = None


@dataclass
class ParsedTranscript:
    """Structured representation of a Claude Code transcript"""
    session_id: str
    user_query: str
    tool_events: List[ToolEventLite]
    stdout: str
    stderr: str
    shown_qa_ids: List[str]
    used_qa_ids: List[str]
    exit_code: int
    duration_ms: int

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization"""
        return {
            'session_id': self.session_id,
            'user_query': self.user_query,
            'tool_events': [asdict(e) for e in self.tool_events],
            'stdout': self.stdout,
            'stderr': self.stderr,
            'shown_qa_ids': self.shown_qa_ids,
            'used_qa_ids': self.used_qa_ids,
            'exit_code': self.exit_code,
            'duration_ms': self.duration_ms,
        }


class TranscriptParser:
    """Parser for Claude Code JSONL transcript files"""

    def __init__(self, transcript_path: str, session_id: str):
        self.transcript_path = transcript_path
        self.session_id = session_id

    def parse(self) -> ParsedTranscript:
        """
        Parse the transcript file and return structured data

        Returns:
            ParsedTranscript object with all extracted data

        Raises:
            FileNotFoundError: If transcript file doesn't exist
            ValueError: If transcript format is invalid
        """
        if not Path(self.transcript_path).exists():
            raise FileNotFoundError(f"Transcript file not found: {self.transcript_path}")

        user_query = ""
        tool_events = []
        stdout_parts = []
        stderr_parts = []
        exit_code = 0
        duration_ms = 0

        # Map tool_use_id to ToolEventLite for matching with tool_result
        pending_tools = {}

        try:
            with open(self.transcript_path, 'r', encoding='utf-8') as f:
                for line_num, line in enumerate(f, 1):
                    line = line.strip()
                    if not line:
                        continue

                    try:
                        event = json.loads(line)
                        event_type = event.get('type', '')

                        # Extract user query from first user message
                        if event_type == 'user' and not user_query:
                            msg = event.get('message', {})
                            content = msg.get('content', '')

                            # Handle string content (direct query)
                            if isinstance(content, str):
                                user_query = content
                            # Handle array content (check for non-tool_result items)
                            elif isinstance(content, list):
                                for item in content:
                                    if isinstance(item, dict) and item.get('type') != 'tool_result':
                                        # Extract text from first non-tool_result item
                                        user_query = item.get('text', item.get('content', ''))
                                        if user_query:
                                            break

                        # Extract assistant outputs and tool uses
                        elif event_type == 'assistant':
                            msg = event.get('message', {})
                            content = msg.get('content', [])

                            if isinstance(content, list):
                                for item in content:
                                    if not isinstance(item, dict):
                                        continue

                                    item_type = item.get('type', '')

                                    # Extract text output
                                    if item_type == 'text':
                                        text = item.get('text', '')
                                        if text:
                                            stdout_parts.append(text)

                                    # Extract tool use
                                    elif item_type == 'tool_use':
                                        tool_id = item.get('id', '')
                                        tool_name = item.get('name', 'unknown')
                                        tool_args = item.get('input', {})

                                        # Create tool event and store for later matching
                                        tool_event = ToolEventLite(
                                            tool=tool_name,
                                            args=tool_args,
                                            output=None,
                                            code=None
                                        )
                                        pending_tools[tool_id] = tool_event

                                    # Assistant thinking can be optionally recorded
                                    elif item_type == 'thinking':
                                        pass  # Ignore for now

                        # Extract tool results (in user messages)
                        elif event_type == 'user':
                            msg = event.get('message', {})
                            content = msg.get('content', [])

                            if isinstance(content, list):
                                for item in content:
                                    if not isinstance(item, dict):
                                        continue

                                    if item.get('type') == 'tool_result':
                                        tool_use_id = item.get('tool_use_id', '')
                                        is_error = item.get('is_error', False)
                                        result_content = item.get('content', '')

                                        # Match with pending tool use
                                        if tool_use_id in pending_tools:
                                            tool_event = pending_tools[tool_use_id]
                                            tool_event.output = result_content
                                            tool_event.code = 1 if is_error else 0
                                            tool_events.append(tool_event)
                                            del pending_tools[tool_use_id]

                        # Extract session end info (if present)
                        elif event_type == 'session.end':
                            exit_code = event.get('exit_code', 0)
                            duration_ms = event.get('duration_ms', 0)

                    except json.JSONDecodeError as e:
                        # Skip invalid lines but log the error
                        print(f"Warning: Invalid JSON at line {line_num}: {e}", file=sys.stderr)
                        continue

        except Exception as e:
            raise ValueError(f"Failed to parse transcript: {e}")

        # Combine stdout parts
        stdout = '\n'.join(stdout_parts)

        # Read shown_qa_ids from temporary file
        shown_qa_ids = self._read_shown_qa_ids()

        # Extract used_qa_ids from stdout
        used_qa_ids = self._extract_qa_refs(stdout)

        return ParsedTranscript(
            session_id=self.session_id,
            user_query=user_query,
            tool_events=tool_events,
            stdout=stdout,
            stderr=''.join(stderr_parts),
            shown_qa_ids=shown_qa_ids,
            used_qa_ids=used_qa_ids,
            exit_code=exit_code,
            duration_ms=duration_ms,
        )

    def _read_shown_qa_ids(self) -> List[str]:
        """
        Read shown_qa_ids from temporary file created by SessionStart hook

        Returns:
            List of QA IDs that were shown to the user
        """
        # Prefer shared session_state (written by memory-inject hook)
        try:
            state = load_session_state(self.session_id)
            shown = state.get("shown_qa_ids", [])
            if isinstance(shown, list) and shown:
                return [str(x) for x in shown if str(x).strip()]
        except Exception:
            pass

        # Check multiple possible locations
        possible_paths = [
            Path(f"/tmp/claude_session_{self.session_id}_shown_qa_ids.txt"),
            Path.home().joinpath(".memex", "sessions", f"{self.session_id}_shown_qa_ids.txt"),
            Path(f"C:/temp/claude_session_{self.session_id}_shown_qa_ids.txt"),  # Windows
        ]

        for path in possible_paths:
            if path.exists():
                try:
                    content = path.read_text(encoding='utf-8').strip()
                    if content:
                        # Split by comma and filter empty strings
                        qa_ids = [qa_id.strip() for qa_id in content.split(',') if qa_id.strip()]
                        return qa_ids
                except Exception:
                    continue

        return []

    def _extract_qa_refs(self, stdout: str) -> List[str]:
        """
        Extract QA references from stdout using regex patterns

        Supports patterns:
        - <!-- memex-qa:ID -->  (preferred)

        Args:
            stdout: The stdout content to search

        Returns:
            List of QA IDs that were referenced in the output
        """
        if not stdout:
            return []

        # Preferred: extract from HTML comment markers injected by memex
        comment_pattern = r'<!--\s*memex-qa:([a-zA-Z0-9-]+)\s*-->'
        matches = re.findall(comment_pattern, stdout)
        qa_ids: List[str] = []
        for qa_id in matches:
            if qa_id and qa_id not in qa_ids:
                qa_ids.append(qa_id)
        return qa_ids


def parse_transcript(transcript_path: str, session_id: str) -> ParsedTranscript:
    """
    Convenience function to parse a transcript file

    Args:
        transcript_path: Path to the JSONL transcript file
        session_id: Session ID for this transcript

    Returns:
        ParsedTranscript object

    Example:
        >>> parsed = parse_transcript('/tmp/session_123.jsonl', 'session_123')
        >>> print(parsed.user_query)
        >>> print(f"Exit code: {parsed.exit_code}")
        >>> print(f"Tools used: {len(parsed.tool_events)}")
    """
    parser = TranscriptParser(transcript_path, session_id)
    return parser.parse()


if __name__ == '__main__':
    import sys

    if len(sys.argv) < 3:
        print("Usage: transcript_parser.py <transcript_file> <session_id>")
        sys.exit(1)

    transcript_file = sys.argv[1]
    session_id = sys.argv[2]

    try:
        parsed = parse_transcript(transcript_file, session_id)
        print(json.dumps(parsed.to_dict(), indent=2, ensure_ascii=False))
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
