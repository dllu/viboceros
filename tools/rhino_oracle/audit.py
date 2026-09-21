"""Complete saved-observation replay, with execution failures kept distinct.

Normal comparison still requires successful engine responses. This diagnostic
schema never turns a native error into a value, zero residual, or timing sample.
"""
from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any, Mapping

from .client import (
    PROTOCOL_VERSION, OperationComparison, OracleProtocolError,
    _validate_epsilon, _validate_response, compare_responses,
)

ERROR_KINDS = frozenset((
    "geometry", "document", "command", "interchange", "io", "json",
    "input", "timing", "fixture_invariant",
))


@dataclass(frozen=True)
class NativeFailure:
    kind: str
    message: str


@dataclass(frozen=True)
class AuditOperationComparison:
    id: str
    comparison: OperationComparison | None
    native_error: NativeFailure | None

    @property
    def passed(self) -> bool:
        return self.native_error is None and self.comparison is not None and self.comparison.passed


@dataclass(frozen=True)
class AuditReport:
    protocol_version: int
    absolute_epsilon: float
    relative_epsilon: float
    operations: tuple[AuditOperationComparison, ...]

    @property
    def matched(self) -> int:
        return sum(row.passed for row in self.operations)

    @property
    def native_failed(self) -> int:
        return sum(row.native_error is not None for row in self.operations)

    @property
    def passed(self) -> bool:
        return self.matched == len(self.operations)

    def as_dict(self) -> dict[str, Any]:
        result = asdict(self)
        result.update(
            passed=self.passed, cases=len(self.operations), matched=self.matched,
            native_failed=self.native_failed,
            mismatched=len(self.operations) - self.matched - self.native_failed,
            timing_note="Comparison timings are harness measurements, not kernel speedups; failed operations have no timing or numeric comparison.",
        )
        for row, serialized in zip(self.operations, result["operations"]):
            serialized["passed"] = row.passed
        return result


def validate_audit_response(response: Mapping[str, Any]) -> None:
    if not isinstance(response, Mapping):
        raise OracleProtocolError("native audit response must be an object")
    if type(response.get("protocol_version")) is not int or response["protocol_version"] != PROTOCOL_VERSION:
        raise OracleProtocolError("unsupported native audit protocol version")
    if response.get("engine") != "viboceros" or not isinstance(response.get("engine_version"), str):
        raise OracleProtocolError("invalid native audit engine")
    if "results" in response or "error" in response:
        raise OracleProtocolError("native audit must contain outcomes, not a normal response or global error")
    outcomes = response.get("outcomes")
    if not isinstance(outcomes, list):
        raise OracleProtocolError("native audit outcomes must be an array")
    successes, seen = [], set()
    for outcome in outcomes:
        if not isinstance(outcome, Mapping):
            raise OracleProtocolError("native audit outcome must be an object")
        status = outcome.get("status")
        if status == "success" and set(outcome) == {"status", "result"}:
            result = outcome["result"]
            if not isinstance(result, Mapping) or set(result) != {"id", "value", "elapsed_ns"}:
                raise OracleProtocolError("native audit success must contain a result")
            operation_id = result.get("id")
            successes.append(result)
        elif status == "failure" and set(outcome) == {"status", "id", "error"}:
            operation_id = outcome["id"]
            error = outcome["error"]
            if (not isinstance(error, Mapping) or set(error) != {"kind", "message"}
                    or not isinstance(error["kind"], str) or error["kind"] not in ERROR_KINDS
                    or not isinstance(error["message"], str) or not error["message"].strip()):
                raise OracleProtocolError("native audit failure needs a known kind and nonempty message")
        else:
            raise OracleProtocolError("invalid or contradictory native audit outcome")
        if not isinstance(operation_id, str) or not operation_id.strip() or operation_id in seen:
            raise OracleProtocolError("native audit outcome ids must be nonempty and unique")
        seen.add(operation_id)
    _validate_response(_success_response(response, successes), "viboceros")


def _success_response(response, successes):
    return {key: response.get(key) for key in (
        "protocol_version", "engine", "engine_version", "iterations",
    )} | {"results": successes}


def validate_replay_inputs(request, observation, absolute_epsilon, relative_epsilon):
    """Reject mismatched records and invalid epsilons before starting a process."""
    _validate_epsilon(absolute_epsilon, "absolute")
    _validate_epsilon(relative_epsilon, "relative")
    _validate_response(observation, "rhino")
    if (not isinstance(request, Mapping) or type(request.get("protocol_version")) is not int
            or request["protocol_version"] != observation["protocol_version"]):
        raise OracleProtocolError("replay request and observation protocol versions must match")
    iterations = request.get("iterations", 1)
    if type(iterations) is not int or not 1 <= iterations <= 1_000_000 or iterations != observation["iterations"]:
        raise OracleProtocolError("replay request and observation iteration counts must match")
    operations = request.get("operations")
    if not isinstance(operations, list) or any(not isinstance(o, Mapping) for o in operations):
        raise OracleProtocolError("replay request operations must be an array of objects")
    ids = [o.get("id") for o in operations]
    if any(not isinstance(i, str) or not i.strip() for i in ids) or len(set(ids)) != len(ids):
        raise OracleProtocolError("replay request ids must be nonempty and unique")
    if set(ids) != {o["id"] for o in observation["results"]}:
        raise OracleProtocolError("replay request and observation ids do not match")


def compare_audit_response(
    native: Mapping[str, Any], observation: Mapping[str, Any],
    absolute_epsilon: float = 1.0e-10, relative_epsilon: float = 1.0e-10,
) -> AuditReport:
    validate_audit_response(native)
    _validate_response(observation, "rhino")
    ids = [o["result"]["id"] if o["status"] == "success" else o["id"] for o in native["outcomes"]]
    if set(ids) != {o["id"] for o in observation["results"]}:
        raise OracleProtocolError("audit and observation ids do not match")
    successes = [o["result"] for o in native["outcomes"] if o["status"] == "success"]
    success_ids = {o["id"] for o in successes}
    comparison = compare_responses(
        _success_response(native, successes),
        dict(observation, results=[o for o in observation["results"] if o["id"] in success_ids]),
        absolute_epsilon, relative_epsilon,
    )
    matches = {o.id: o for o in comparison.operations}
    rows = []
    for operation_id, outcome in zip(ids, native["outcomes"]):
        failure = NativeFailure(**outcome["error"]) if outcome["status"] == "failure" else None
        rows.append(AuditOperationComparison(operation_id, matches.get(operation_id), failure))
    return AuditReport(PROTOCOL_VERSION, absolute_epsilon, relative_epsilon, tuple(rows))
