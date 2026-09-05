"""Host-side API for the Viboceros/Rhino geometry oracle protocol."""

from __future__ import annotations

import copy
import json
import math
import os
import signal
import shutil
import subprocess
import tempfile
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Mapping, Sequence

PROTOCOL_VERSION = 1
DEFAULT_LAUNCHER = Path.home() / "wines/prefixes/rhino/launch.sh"
RHINO_EXIT_GRACE_SECONDS = 15.0


class OracleError(RuntimeError):
    """Base error raised by the host-side oracle driver."""


class OracleProtocolError(OracleError):
    """Raised when an engine returns an invalid protocol response."""


@dataclass(frozen=True)
class OperationComparison:
    """Correctness and timing comparison for one geometry operation."""

    id: str
    passed: bool
    max_absolute_error: float
    viboceros_ns_per_iteration: float
    rhino_ns_per_iteration: float
    rhino_to_viboceros_ratio: float | None
    differences: tuple[str, ...]


@dataclass(frozen=True)
class ComparisonReport:
    """Comparison of two complete protocol responses."""

    protocol_version: int
    absolute_epsilon: float
    relative_epsilon: float
    passed: bool
    operations: tuple[OperationComparison, ...]

    @property
    def max_absolute_error(self) -> float:
        """Return the largest numeric difference in the batch."""

        return max(
            (operation.max_absolute_error for operation in self.operations),
            default=0.0,
        )

    def as_dict(self) -> dict[str, Any]:
        """Return a JSON-serializable representation."""

        result = asdict(self)
        result["max_absolute_error"] = self.max_absolute_error
        return result


def load_request(path: str | os.PathLike[str]) -> dict[str, Any]:
    """Load a JSON request and require an object at its root."""

    with Path(path).open("r", encoding="utf-8") as stream:
        request = json.load(stream)
    if not isinstance(request, dict):
        raise OracleProtocolError("oracle request root must be a JSON object")
    return request


def posix_to_wine_path(path: str | os.PathLike[str]) -> str:
    """Map an absolute POSIX path through Wine's conventional Z: drive."""

    resolved = Path(path).resolve()
    if not resolved.is_absolute():  # pragma: no cover - resolve is absolute
        raise OracleError(f"oracle path must be absolute: {path}")
    return "Z:" + str(resolved).replace("/", "\\")


class OracleClient:
    """Run matching probes in native Viboceros and Rhino 8."""

    def __init__(
        self,
        repo_root: str | os.PathLike[str] | None = None,
        launcher: str | os.PathLike[str] | None = None,
    ) -> None:
        self.repo_root = (
            Path(repo_root).resolve()
            if repo_root is not None
            else Path(__file__).resolve().parents[2]
        )
        configured_launcher = launcher or os.environ.get(
            "VIBOCEROS_RHINO_LAUNCHER", DEFAULT_LAUNCHER
        )
        self.launcher = Path(configured_launcher).expanduser().resolve()

    def run_viboceros(
        self, request: Mapping[str, Any], timeout: float = 180.0
    ) -> dict[str, Any]:
        """Run the native release-mode Rust probe."""

        with tempfile.TemporaryDirectory(prefix="viboceros-oracle-") as job:
            job_path = Path(job)
            request_path = job_path / "request.json"
            response_path = job_path / "response.json"
            _write_json(request_path, request)
            command = [
                "cargo",
                "run",
                "--quiet",
                "--release",
                "--package",
                "viboceros-oracle",
                "--",
                str(request_path),
                str(response_path),
            ]
            completed = _run(command, self.repo_root, timeout)
            if completed.returncode != 0:
                raise OracleError(_command_failure("Viboceros probe", completed))
            if not response_path.is_file():
                raise OracleError("Viboceros probe completed without a response")
            response = _read_json(response_path)
        _validate_response(response, "viboceros")
        return response

    def run_rhino(
        self, request: Mapping[str, Any], timeout: float = 180.0
    ) -> dict[str, Any]:
        """Launch a Rhino Python worker and wait for its atomic response.

        Rhino's documented ``/runscript`` startup argument is used first. The
        project's Wine/FEX launcher currently drops that macro, so on Linux an
        owned, newly-created Rhino window is used for a scoped xdotool fallback.
        Existing Rhino processes and windows are never targeted.
        """

        if not self.launcher.is_file():
            raise OracleError(f"Rhino launcher not found: {self.launcher}")
        worker_source = Path(__file__).with_name("rhino_worker.py")
        if not worker_source.is_file():
            raise OracleError(f"Rhino worker not found: {worker_source}")

        with tempfile.TemporaryDirectory(prefix="viboceros-rhino-oracle-") as job:
            job_path = Path(job)
            request_path = job_path / "request.json"
            response_path = job_path / "response.json"
            worker_path = job_path / "rhino_worker.py"
            worker_request = dict(request)
            worker_request["_host"] = {"exit_rhino_when_complete": True}
            _write_json(request_path, worker_request)
            shutil.copyfile(worker_source, worker_path)
            windows_worker = posix_to_wine_path(worker_path)
            # The generated temporary path never contains spaces, so use the
            # exact no-parentheses form documented by McNeel for /runscript.
            macro = f"-_RunPythonScript {windows_worker}"
            command = [
                str(self.launcher),
                "launch",
                "/nosplash",
                f"/runscript={macro}",
            ]
            existing_pids = _rhino_process_ids()
            launch_timeout = min(timeout, 60.0)
            completed = _run_logged(
                command,
                self.repo_root,
                launch_timeout,
                job_path / "launcher-output.log",
            )
            if completed.returncode != 0:
                raise OracleError(_command_failure("Rhino launcher", completed))

            deadline = time.monotonic() + timeout
            owned_pids: set[int] = set()
            owned_window: str | None = None
            fallback_ready_at: float | None = None
            fallback_sent = False
            try:
                while not response_path.is_file() and time.monotonic() < deadline:
                    owned_pids.update(
                        _rhino_process_ids(windows_worker) - existing_pids
                    )
                    if not fallback_sent and _ui_fallback_enabled():
                        candidate = _rhino_window_for_pids(owned_pids)
                        if candidate is not None and owned_window != candidate:
                            owned_window = candidate
                            # A Rhino window acquires its final title before
                            # its command line reliably accepts keystrokes.
                            fallback_ready_at = time.monotonic() + 3.0
                        if (
                            candidate is not None
                            and fallback_ready_at is not None
                            and time.monotonic() >= fallback_ready_at
                        ):
                            _send_rhino_macro(
                                candidate, macro, self.repo_root, min(10.0, timeout)
                            )
                            fallback_sent = True
                    time.sleep(0.05)
                if not response_path.is_file():
                    details = _command_output(completed)
                    progress = _read_optional_text(job_path / "worker-progress.log")
                    diagnostics = []
                    if progress:
                        diagnostics.append(f"Worker progress:\n{progress}")
                    if details:
                        diagnostics.append(f"Launcher output:\n{details}")
                    suffix = (
                        "\n" + "\n".join(diagnostics) if diagnostics else ""
                    )
                    raise OracleError(
                        f"Rhino probe did not respond within {timeout:g} seconds "
                        f"(owned_pids={sorted(owned_pids)}, "
                        f"owned_window={owned_window!r}, "
                        f"ui_fallback_sent={fallback_sent}){suffix}"
                    )
                response = _read_json(response_path)
            finally:
                owned_pids.update(_rhino_process_ids(windows_worker) - existing_pids)
                if response_path.is_file() and _wait_for_process_exit(
                    owned_pids, RHINO_EXIT_GRACE_SECONDS
                ):
                    owned_window = None
                if owned_window is None:
                    owned_window = _rhino_window_for_pids(owned_pids)
                if owned_window is not None:
                    _close_rhino_window(owned_window, self.repo_root)
                _terminate_owned_rhino_processes(owned_pids, windows_worker)
        _validate_response(response, "rhino")
        return response

    def compare(
        self,
        request: Mapping[str, Any],
        absolute_epsilon: float = 1.0e-10,
        relative_epsilon: float = 1.0e-10,
        timeout: float = 180.0,
    ) -> ComparisonReport:
        """Run both engines and compare every numeric result within epsilon."""

        # Cross-reader probes must inspect the same actual file. Keep artifacts
        # alive through both engines, and never mutate the caller's fixture.
        with tempfile.TemporaryDirectory(prefix="viboceros-interchange-") as job:
            prepared = copy.deepcopy(dict(request))
            for index, operation in enumerate(prepared.get("operations", [])):
                if operation.get("op") == "three_dm_curve_interchange":
                    operation["artifact_path"] = str(Path(job) / f"curve-{index}.3dm")
            viboceros = self.run_viboceros(prepared, timeout)
            rhino = self.run_rhino(prepared, timeout)
        return compare_responses(
            viboceros,
            rhino,
            absolute_epsilon=absolute_epsilon,
            relative_epsilon=relative_epsilon,
        )


def compare_responses(
    viboceros: Mapping[str, Any],
    rhino: Mapping[str, Any],
    absolute_epsilon: float = 1.0e-10,
    relative_epsilon: float = 1.0e-10,
) -> ComparisonReport:
    """Compare validated engine responses without launching either engine."""

    _validate_epsilon(absolute_epsilon, "absolute")
    _validate_epsilon(relative_epsilon, "relative")
    _validate_response(viboceros, "viboceros")
    _validate_response(rhino, "rhino")
    if viboceros["protocol_version"] != rhino["protocol_version"]:
        raise OracleProtocolError("engine protocol versions do not match")
    if viboceros["iterations"] != rhino["iterations"]:
        raise OracleProtocolError("engine iteration counts do not match")

    v_results = _result_map(viboceros)
    r_results = _result_map(rhino)
    if set(v_results) != set(r_results):
        missing = sorted(set(v_results) - set(r_results))
        extra = sorted(set(r_results) - set(v_results))
        raise OracleProtocolError(
            f"engine result ids do not match; missing={missing}, extra={extra}"
        )

    iterations = int(viboceros["iterations"])
    comparisons = []
    for result in viboceros["results"]:
        operation_id = result["id"]
        rhino_result = r_results[operation_id]
        differences: list[str] = []
        max_error = _compare_value(
            f"{operation_id}.value",
            result["value"],
            rhino_result["value"],
            absolute_epsilon,
            relative_epsilon,
            differences,
        )
        viboceros_ns = float(result["elapsed_ns"]) / iterations
        rhino_ns = float(rhino_result["elapsed_ns"]) / iterations
        ratio = rhino_ns / viboceros_ns if viboceros_ns > 0.0 else None
        comparisons.append(
            OperationComparison(
                id=operation_id,
                passed=not differences,
                max_absolute_error=max_error,
                viboceros_ns_per_iteration=viboceros_ns,
                rhino_ns_per_iteration=rhino_ns,
                rhino_to_viboceros_ratio=ratio,
                differences=tuple(differences),
            )
        )
    operations = tuple(comparisons)
    return ComparisonReport(
        protocol_version=int(viboceros["protocol_version"]),
        absolute_epsilon=absolute_epsilon,
        relative_epsilon=relative_epsilon,
        passed=all(operation.passed for operation in operations),
        operations=operations,
    )


def _run(command: Sequence[str], cwd: Path, timeout: float) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise OracleError(
            f"command timed out after {timeout:g} seconds: {command[0]}"
        ) from error
    except OSError as error:
        raise OracleError(f"could not run {command[0]}: {error}") from error


def _run_logged(
    command: Sequence[str], cwd: Path, timeout: float, log_path: Path
) -> subprocess.CompletedProcess[str]:
    """Run a launcher without waiting for inherited output pipes to close.

    Wine services can outlive the launcher and inherit its file descriptors.
    A regular log file retains diagnostics without making ``subprocess.run``
    wait for those descendants to close a captured pipe.
    """

    try:
        with log_path.open("w+", encoding="utf-8") as stream:
            completed = subprocess.run(
                command,
                cwd=cwd,
                text=True,
                stdout=stream,
                stderr=subprocess.STDOUT,
                timeout=timeout,
                check=False,
            )
            stream.flush()
            stream.seek(0)
            output = stream.read()
        return subprocess.CompletedProcess(
            completed.args,
            completed.returncode,
            stdout=output,
            stderr="",
        )
    except subprocess.TimeoutExpired as error:
        raise OracleError(
            f"command timed out after {timeout:g} seconds: {command[0]}"
        ) from error
    except OSError as error:
        raise OracleError(f"could not run {command[0]}: {error}") from error


def _rhino_process_ids(command_marker: str | None = None) -> set[int]:
    """Return matching Rhino.exe PIDs visible through procfs."""

    proc = Path("/proc")
    if not proc.is_dir():
        return set()
    result = set()
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            command_line = (entry / "cmdline").read_bytes()
        except OSError:
            continue
        arguments = command_line.split(b"\0")
        command = arguments[0]
        executable = command.decode("utf-8", errors="replace").replace("\\", "/")
        decoded_command_line = command_line.decode("utf-8", errors="replace")
        if executable.lower().endswith("/rhino.exe") and (
            command_marker is None or command_marker in decoded_command_line
        ):
            result.add(int(entry.name))
    return result


def _rhino_window_for_pids(pids: set[int]) -> str | None:
    """Find an initialized Rhino 8 X11 window owned by one of the PIDs."""

    wmctrl = shutil.which("wmctrl")
    if not pids or wmctrl is None:
        return None
    try:
        completed = subprocess.run(
            [wmctrl, "-lp"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=5.0,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if completed.returncode != 0:
        return None
    for line in completed.stdout.splitlines():
        fields = line.split(maxsplit=4)
        if len(fields) < 5:
            continue
        try:
            pid = int(fields[2])
        except ValueError:
            continue
        if pid in pids and "Rhino 8" in fields[4]:
            return fields[0]
    return None


def _wait_for_process_exit(pids: set[int], timeout: float) -> bool:
    if not pids or not Path("/proc").is_dir():
        return not pids
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not any((Path("/proc") / str(pid)).exists() for pid in pids):
            return True
        time.sleep(0.05)
    return not any((Path("/proc") / str(pid)).exists() for pid in pids)


def _terminate_owned_rhino_processes(
    pids: set[int], command_marker: str
) -> None:
    """Stop only Rhino processes carrying this oracle worker path."""

    targets = pids & _rhino_process_ids(command_marker)
    for pid in targets:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    if _wait_for_process_exit(targets, 2.0):
        return
    for pid in targets & _rhino_process_ids(command_marker):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    _wait_for_process_exit(targets, 2.0)


def _ui_fallback_enabled() -> bool:
    return (
        os.environ.get("VIBOCEROS_RHINO_UI_FALLBACK", "1") != "0"
        and shutil.which("wmctrl") is not None
        and shutil.which("xdotool") is not None
    )


def _send_rhino_macro(window: str, macro: str, cwd: Path, timeout: float) -> None:
    xdotool = shutil.which("xdotool")
    if xdotool is None:  # pragma: no cover - guarded by _ui_fallback_enabled
        raise OracleError("xdotool is required for the Rhino Wine UI fallback")
    command_name, separator, script_path = macro.partition(" ")
    if not separator or not command_name or not script_path:
        raise OracleError("Rhino UI fallback received an invalid command macro")
    commands = [
        [xdotool, "windowactivate", "--sync", window],
        [xdotool, "key", "Escape"],
        [
            xdotool,
            "type",
            "--clearmodifiers",
            "--delay",
            "5",
            "--",
            command_name,
        ],
        [xdotool, "key", "Return"],
        [
            xdotool,
            "type",
            "--clearmodifiers",
            "--delay",
            "5",
            "--",
            script_path,
        ],
        [xdotool, "key", "Return"],
    ]
    for index, command in enumerate(commands):
        completed = _run(command, cwd, timeout)
        if completed.returncode != 0:
            raise OracleError(_command_failure("Rhino UI fallback", completed))
        # Let focus settle after activation and let RunPythonScript open its
        # file-name prompt before typing the path.
        if index == 0:
            time.sleep(2.0)
        elif index == 3:
            time.sleep(1.0)


def _close_rhino_window(window: str, cwd: Path) -> None:
    xdotool = shutil.which("xdotool")
    if xdotool is None:
        return
    # windowclose sends the normal WM_DELETE request; it does not kill Rhino.
    try:
        subprocess.run(
            [xdotool, "windowclose", window],
            cwd=cwd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=5.0,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        pass


def _write_json(path: Path, value: Mapping[str, Any]) -> None:
    with path.open("w", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def _read_json(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as stream:
            value = json.load(stream)
    except (OSError, json.JSONDecodeError) as error:
        raise OracleProtocolError(f"could not read oracle response {path}: {error}") from error
    if not isinstance(value, dict):
        raise OracleProtocolError("oracle response root must be a JSON object")
    return value


def _read_optional_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace").strip()
    except OSError:
        return ""


def _validate_response(response: Mapping[str, Any], engine: str) -> None:
    if not isinstance(response, Mapping):
        raise OracleProtocolError(f"{engine} response must be a JSON object")
    if response.get("protocol_version") != PROTOCOL_VERSION:
        raise OracleProtocolError(
            f"{engine} returned protocol version {response.get('protocol_version')!r}; "
            f"expected {PROTOCOL_VERSION}"
        )
    if response.get("engine") != engine:
        raise OracleProtocolError(
            f"expected {engine} response, got {response.get('engine')!r}"
        )
    error = response.get("error")
    if error is not None:
        raise OracleError(f"{engine} probe failed: {error}")
    iterations = response.get("iterations")
    if isinstance(iterations, bool) or not isinstance(iterations, int) or iterations < 1:
        raise OracleProtocolError(f"{engine} returned invalid iteration count")
    results = response.get("results")
    if not isinstance(results, list):
        raise OracleProtocolError(f"{engine} results must be an array")
    seen = set()
    for result in results:
        if not isinstance(result, Mapping):
            raise OracleProtocolError(f"{engine} result must be an object")
        operation_id = result.get("id")
        if not isinstance(operation_id, str) or not operation_id or operation_id in seen:
            raise OracleProtocolError(f"{engine} returned an invalid result id")
        seen.add(operation_id)
        if "value" not in result:
            raise OracleProtocolError(f"{engine} result {operation_id!r} has no value")
        _validate_json_numbers(result["value"], f"{engine}.{operation_id}.value")
        elapsed = result.get("elapsed_ns")
        if isinstance(elapsed, bool) or not isinstance(elapsed, int) or elapsed < 0:
            raise OracleProtocolError(
                f"{engine} result {operation_id!r} has invalid elapsed_ns"
            )


def _result_map(response: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    return {result["id"]: result for result in response["results"]}


def _validate_epsilon(value: float, name: str) -> None:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise OracleProtocolError(f"{name} epsilon must be numeric")
    if not math.isfinite(value) or value < 0.0:
        raise OracleProtocolError(f"{name} epsilon must be finite and non-negative")


def _validate_json_numbers(value: Any, path: str) -> None:
    if _is_number(value):
        try:
            finite = math.isfinite(value)
        except (OverflowError, TypeError):
            finite = False
        if not finite:
            raise OracleProtocolError(f"{path} contains a non-finite number")
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            _validate_json_numbers(child, f"{path}.{key}")
        return
    if _is_sequence(value):
        for index, child in enumerate(value):
            _validate_json_numbers(child, f"{path}[{index}]")


def _compare_value(
    path: str,
    viboceros: Any,
    rhino: Any,
    absolute_epsilon: float,
    relative_epsilon: float,
    differences: list[str],
) -> float:
    if _is_number(viboceros) and _is_number(rhino):
        left = float(viboceros)
        right = float(rhino)
        if not math.isfinite(left) or not math.isfinite(right):
            differences.append(f"{path}: non-finite numeric result")
            return math.inf
        error = abs(left - right)
        limit = max(absolute_epsilon, relative_epsilon * max(abs(left), abs(right)))
        if error > limit:
            differences.append(
                f"{path}: {left:.17g} != {right:.17g} "
                f"(error {error:.3g}, limit {limit:.3g})"
            )
        return error

    if isinstance(viboceros, Mapping) and isinstance(rhino, Mapping):
        left_keys = set(viboceros)
        right_keys = set(rhino)
        if left_keys != right_keys:
            differences.append(
                f"{path}: object keys differ; missing={sorted(left_keys - right_keys)}, "
                f"extra={sorted(right_keys - left_keys)}"
            )
        errors = [
            _compare_value(
                f"{path}.{key}",
                viboceros[key],
                rhino[key],
                absolute_epsilon,
                relative_epsilon,
                differences,
            )
            for key in sorted(left_keys & right_keys)
        ]
        return max(errors, default=0.0)

    if _is_sequence(viboceros) and _is_sequence(rhino):
        if len(viboceros) != len(rhino):
            differences.append(
                f"{path}: array lengths differ ({len(viboceros)} != {len(rhino)})"
            )
        errors = [
            _compare_value(
                f"{path}[{index}]",
                left,
                right,
                absolute_epsilon,
                relative_epsilon,
                differences,
            )
            for index, (left, right) in enumerate(zip(viboceros, rhino))
        ]
        return max(errors, default=0.0)

    if type(viboceros) is not type(rhino) or viboceros != rhino:
        differences.append(f"{path}: {viboceros!r} != {rhino!r}")
    return 0.0


def _is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _is_sequence(value: Any) -> bool:
    return isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray))


def _command_output(completed: subprocess.CompletedProcess[str]) -> str:
    return "\n".join(
        part.strip() for part in (completed.stdout, completed.stderr) if part.strip()
    )


def _command_failure(label: str, completed: subprocess.CompletedProcess[str]) -> str:
    details = _command_output(completed)
    suffix = f"\n{details}" if details else ""
    return f"{label} exited with status {completed.returncode}{suffix}"
