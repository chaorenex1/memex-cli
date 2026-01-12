#!/usr/bin/env python3
"""
Simplified Gatekeeper for Claude Code Integration
Evaluates session outcomes and decides what to record in Memory
"""

from typing import List, Dict, Any, Optional
from dataclasses import dataclass, field


@dataclass
class SearchMatch:
    """Memory search result"""
    qa_id: str
    question: str
    answer: str
    score: float
    trust: float
    validation_level: int = 0
    tags: List[str] = field(default_factory=list)


@dataclass
class QAReference:
    """Reference to a QA item"""
    qa_id: str
    shown: bool
    used: bool
    message_id: Optional[str] = None
    context: Optional[str] = None


@dataclass
class ValidationPlan:
    """Plan to validate a QA item"""
    qa_id: str
    success: bool
    confidence: float = 0.8


@dataclass
class GatekeeperDecision:
    """Decision on what to record in Memory"""
    # Hit recording
    hit_refs: List[QAReference] = field(default_factory=list)

    # Validation plans
    validate_plans: List[ValidationPlan] = field(default_factory=list)

    # Candidate recording
    should_write_candidate: bool = False
    candidate_confidence: float = 0.7

    # Metadata
    quality_score: float = 0.0
    reasons: List[str] = field(default_factory=list)


class SimpleGatekeeper:
    """
    Simplified Gatekeeper implementation for Python hooks

    This is a lightweight version that implements core logic:
    - Decides whether to record Hit (shown/used QA items)
    - Decides whether to record Validation (successful usage)
    - Decides whether to record Candidate (new knowledge)
    """

    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        Initialize Gatekeeper with configuration

        Args:
            config: Configuration dict with keys:
                - min_trust_show: Minimum trust score to show (default: 0.4)
                - max_inject: Maximum items to inject (default: 3)
                - min_quality_score: Minimum quality to record candidate (default: 0.5)
                - validate_on_success: Validate used QAs on success (default: True)
        """
        self.config = config or {}
        self.min_trust_show = self.config.get('min_trust_show', 0.4)
        self.max_inject = self.config.get('max_inject', 3)
        self.min_quality_score = self.config.get('min_quality_score', 0.5)
        self.validate_on_success = self.config.get('validate_on_success', True)

    def evaluate(
        self,
        matches: List[SearchMatch],
        shown_qa_ids: List[str],
        used_qa_ids: List[str],
        exit_code: int,
        tool_events: List[Any],
        stdout: str,
    ) -> GatekeeperDecision:
        """
        Evaluate session outcome and decide what to record

        Args:
            matches: Search matches that were available
            shown_qa_ids: QA IDs that were shown to user
            used_qa_ids: QA IDs that were referenced in output
            exit_code: Session exit code (0 = success)
            tool_events: List of tool events
            stdout: Session stdout

        Returns:
            GatekeeperDecision with recording decisions
        """
        decision = GatekeeperDecision()

        # 1. Build Hit references
        decision.hit_refs = self._build_hit_refs(shown_qa_ids, used_qa_ids)

        # 2. Build Validation plans (if successful and used QAs)
        if exit_code == 0 and self.validate_on_success:
            decision.validate_plans = self._build_validate_plans(
                matches, used_qa_ids, tool_events
            )

        # 3. Assess quality and decide on Candidate recording
        quality_score, reasons = self._assess_quality(
            exit_code, tool_events, stdout, used_qa_ids
        )
        decision.quality_score = quality_score
        decision.reasons = reasons
        decision.should_write_candidate = quality_score >= self.min_quality_score

        # Adjust candidate confidence based on quality
        if decision.should_write_candidate:
            decision.candidate_confidence = min(0.9, 0.6 + quality_score * 0.3)

        return decision

    def _build_hit_refs(
        self,
        shown_qa_ids: List[str],
        used_qa_ids: List[str]
    ) -> List[QAReference]:
        """
        Build QA reference list for Hit recording

        Args:
            shown_qa_ids: QA IDs shown to user
            used_qa_ids: QA IDs referenced in output

        Returns:
            List of QAReference objects
        """
        refs = []

        # All shown QAs should be recorded
        for qa_id in shown_qa_ids:
            refs.append(QAReference(
                qa_id=qa_id,
                shown=True,
                used=(qa_id in used_qa_ids)
            ))

        # Add any used QAs that weren't shown (edge case)
        for qa_id in used_qa_ids:
            if qa_id not in shown_qa_ids:
                refs.append(QAReference(
                    qa_id=qa_id,
                    shown=False,
                    used=True
                ))

        return refs

    def _build_validate_plans(
        self,
        matches: List[SearchMatch],
        used_qa_ids: List[str],
        tool_events: List[Any],
    ) -> List[ValidationPlan]:
        """
        Build validation plans for successfully used QAs

        Args:
            matches: Available search matches
            used_qa_ids: QA IDs that were used
            tool_events: Tool events from session

        Returns:
            List of ValidationPlan objects
        """
        plans = []

        # Create validation plan for each used QA
        for qa_id in used_qa_ids:
            # Find the match
            match = next((m for m in matches if m.qa_id == qa_id), None)

            # Determine confidence based on match score
            confidence = 0.8
            if match:
                # Higher match score = higher validation confidence
                confidence = min(0.95, 0.7 + match.score * 0.25)

            plans.append(ValidationPlan(
                qa_id=qa_id,
                success=True,  # Assuming success since exit_code was 0
                confidence=confidence
            ))

        return plans

    def _assess_quality(
        self,
        exit_code: int,
        tool_events: List[Any],
        stdout: str,
        used_qa_ids: List[str]
    ) -> tuple[float, List[str]]:
        """
        Assess session quality to decide if new knowledge should be recorded

        Args:
            exit_code: Session exit code
            tool_events: Tool events from session
            stdout: Session output
            used_qa_ids: QA IDs that were used

        Returns:
            Tuple of (quality_score, reasons)
            - quality_score: 0.0 to 1.0
            - reasons: List of reason strings
        """
        score = 0.0
        reasons = []

        # Factor 1: Success (40%)
        if exit_code == 0:
            score += 0.4
            reasons.append("Session succeeded")
        else:
            reasons.append(f"Session failed (exit code: {exit_code})")

        # Factor 2: Tool usage indicates actual work (30%)
        significant_tools = ['Write', 'Edit', 'Bash', 'NotebookEdit']
        tool_count = sum(
            1 for event in tool_events
            if hasattr(event, 'tool') and event.tool in significant_tools
        )

        if tool_count > 0:
            tool_score = min(0.3, tool_count * 0.1)
            score += tool_score
            reasons.append(f"Used {tool_count} significant tools")
        else:
            reasons.append("No significant tool usage")

        # Factor 3: Output substance (20%)
        if stdout:
            # Simple heuristic: substantial output
            output_length = len(stdout)
            if output_length > 500:
                score += 0.2
                reasons.append(f"Substantial output ({output_length} chars)")
            elif output_length > 100:
                score += 0.1
                reasons.append(f"Moderate output ({output_length} chars)")
            else:
                reasons.append(f"Minimal output ({output_length} chars)")

        # Factor 4: Memory utilization (10%)
        if used_qa_ids:
            score += 0.1
            reasons.append(f"Utilized {len(used_qa_ids)} memory items")
        else:
            # Bonus for novel solutions (didn't rely on memory)
            if exit_code == 0 and tool_count > 0:
                score += 0.05
                reasons.append("Novel solution (no memory used)")

        # Cap at 1.0
        score = min(1.0, score)

        return score, reasons


def create_gatekeeper(config: Optional[Dict[str, Any]] = None) -> SimpleGatekeeper:
    """
    Factory function to create a Gatekeeper instance

    Args:
        config: Optional configuration dict

    Returns:
        SimpleGatekeeper instance
    """
    return SimpleGatekeeper(config)


# Example usage
if __name__ == '__main__':
    # Create sample data
    matches = [
        SearchMatch(
            qa_id='qa-123',
            question='How to implement quicksort?',
            answer='Use divide and conquer...',
            score=0.85,
            trust=0.75
        )
    ]

    shown_qa_ids = ['qa-123']
    used_qa_ids = ['qa-123']
    exit_code = 0
    tool_events = []  # Simulated tool events
    stdout = "Implemented quicksort successfully using the reference approach."

    # Evaluate
    gatekeeper = create_gatekeeper()
    decision = gatekeeper.evaluate(
        matches=matches,
        shown_qa_ids=shown_qa_ids,
        used_qa_ids=used_qa_ids,
        exit_code=exit_code,
        tool_events=tool_events,
        stdout=stdout
    )

    print("Gatekeeper Decision:")
    print(f"  Hit refs: {len(decision.hit_refs)}")
    print(f"  Validation plans: {len(decision.validate_plans)}")
    print(f"  Should record candidate: {decision.should_write_candidate}")
    print(f"  Quality score: {decision.quality_score:.2f}")
    print(f"  Reasons: {', '.join(decision.reasons)}")
