"""Host-side API for the Viboceros/Rhino geometry oracle protocol."""

from __future__ import annotations

import copy
import json
import math
import os
import re
import signal
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from contextlib import contextmanager
from pathlib import Path
from typing import TYPE_CHECKING, Any, Mapping, Sequence

if TYPE_CHECKING:
    from .audit import AuditReport

PROTOCOL_VERSION = 1
DEFAULT_LAUNCHER = Path.home() / "wines/prefixes/rhino/launch.sh"
RHINO_EXIT_GRACE_SECONDS = 15.0
RHINO_STARTUP_GRACE_SECONDS = 30.0
RHINO_PROCESS_EXIT_GRACE_SECONDS = 2.0


class OracleError(RuntimeError):
    """Base error raised by the host-side oracle driver."""


class OracleProtocolError(OracleError):
    """Raised when an engine returns an invalid protocol response."""


@dataclass(frozen=True)
class OperationComparison:
    """Correctness and raw harness timings, not a kernel-speed comparison."""

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
    def timing_note(self) -> str:
        """Explain the limits of the reported elapsed-time ratios."""

        return (
            "Timings are harness measurements, not kernel speedups. Timed work "
            "may differ between engines (including geometry extraction and "
            "cleanup); Rhino includes the Python/RhinoCommon bridge and any "
            "host emulation overhead. See docs/oracle.md#timing-interpretation."
        )

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
        result["timing_note"] = self.timing_note
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
        settings_scheme: str | None = None,
    ) -> None:
        if settings_scheme is not None and (
            not isinstance(settings_scheme, str)
            or re.fullmatch(r"VibocerosOracle[A-Za-z0-9_-]{1,64}", settings_scheme) is None
        ):
            raise OracleProtocolError("invalid private Rhino settings scheme")
        self.settings_scheme = settings_scheme
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

        response = self._run_native(request, timeout, audit=False)
        _validate_response(response, "viboceros", request)
        return response

    def run_viboceros_audit(
        self, request: Mapping[str, Any], timeout: float = 180.0
    ) -> dict[str, Any]:
        """Run once, retaining per-operation errors; request/process failures raise."""

        from .audit import validate_audit_response
        response = self._run_native(request, timeout, audit=True)
        validate_audit_response(response)
        return response

    def replay(
        self, request: Mapping[str, Any], observation: Mapping[str, Any],
        absolute_epsilon: float = 1.0e-10, relative_epsilon: float = 1.0e-10,
        timeout: float = 180.0,
    ) -> AuditReport:
        """Audit against saved raw Rhino observations without launching Rhino."""

        from .audit import compare_audit_response, validate_replay_inputs
        validate_replay_inputs(request, observation, absolute_epsilon, relative_epsilon)
        with _owned_artifact_request(request) as prepared:
            native = self.run_viboceros_audit(prepared, timeout)
        return compare_audit_response(native, observation, absolute_epsilon, relative_epsilon)

    def _run_native(
        self, request: Mapping[str, Any], timeout: float, *, audit: bool,
    ) -> dict[str, Any]:

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
                *(["--audit"] if audit else []),
                str(request_path),
                str(response_path),
            ]
            completed = _run(command, self.repo_root, timeout)
            if completed.returncode != 0:
                raise OracleError(_command_failure("Viboceros probe", completed))
            if not response_path.is_file():
                raise OracleError("Viboceros probe completed without a response")
            response = _read_json(response_path)
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

        if (sys.platform.startswith("linux")
                and (not os.environ.get("DISPLAY")
                     or os.environ.get("VIBOCEROS_ORACLE_HEADLESS") != os.environ["DISPLAY"])):
            raise OracleError(
                "live Rhino probes require dedicated Xvfb; use "
                "tools/rhino_oracle/run_headless.sh or the oracle CLI"
            )

        if not self.launcher.is_file():
            raise OracleError(f"Rhino launcher not found: {self.launcher}")
        worker_source = Path(__file__).with_name("rhino_worker.py")
        if any(op.get('op') == 'block_workflow' for op in request.get('operations', [])):
            from .block_workflow_probe import validate
            if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise ValueError('block workflows require one iteration')
            for operation in request['operations']:
                if operation.get('op') == 'block_workflow':
                    validate(operation)
        if any(op.get('op') == 'scale_nu' for op in request.get('operations', [])):
            from .scale_nu_probe import validate
            if self.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise OracleProtocolError('ScaleNU probes require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'scale_nu':
                    validate(op)
        if any(op.get('op') == 'mesh_edit_records' for op in request.get('operations', [])):
            from .mesh_edit_records_probe import validate
            if self.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise OracleProtocolError('Mesh record probes require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'mesh_edit_records':
                    validate(op)
        if any(op.get('op') == 'grip_transform' for op in request.get('operations', [])):
            from .grip_transform_probe import validate
            if self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                raise OracleProtocolError('Grip transforms require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'grip_transform':
                    validate(op)
        if any(op.get('op') in ('circle_fit_grips', 'circle_fit_grips_commands') for op in request.get('operations', [])):
            from .circle_fit_grips_probe import validate
            if self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                raise OracleProtocolError('Circle grip captures require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') in ('circle_fit_grips', 'circle_fit_grips_commands'):
                    validate(op)
        if any(op.get('op') == 'circle_fit_diagnostics' for op in request.get('operations', [])):
            from .circle_fit_diagnostics import validate
            if self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                raise OracleProtocolError('Circle diagnostics require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'circle_fit_diagnostics':
                    validate(op)
        if any(op.get('op') == 'circle_fit_selection' for op in request.get('operations', [])):
            from .circle_fit_selection_probe import validate
            if self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                raise OracleProtocolError('Circle selection captures require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'circle_fit_selection':
                    validate(op)
        if any(op.get('op') == 'circle_fit_benchmark' for op in request.get('operations', [])):
            from .circle_fit_benchmark import validate
            if self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                raise OracleProtocolError('Circle benchmarks require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'circle_fit_benchmark':
                    validate(op)
        if any(op.get('op') == 'circle_fit_points' for op in request.get('operations', [])):
            from .circle_fit_probe import validate
            if self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                raise OracleProtocolError('Circle fit commands require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'circle_fit_points':
                    validate(op)
        for family, module in [('maelstrom_geometry_command', 'maelstrom_command_probe'),
                               ('maelstrom_options_command', 'maelstrom_options_probe'),
                               ('maelstrom_input_command', 'maelstrom_input_probe'),
                               ('maelstrom_circle_command', 'maelstrom_circle_probe'),
                               ('maelstrom_fit_points_command', 'maelstrom_fit_points_probe')]:
            if any(op.get('op') == family for op in request.get('operations', [])):
                from importlib import import_module
                validate = import_module('.'+module, __package__).validate
                if self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                    raise OracleProtocolError('Maelstrom commands require a private scheme and one iteration')
                for op in request['operations']:
                    if op.get('op') == family:
                        validate(op)
        if any(op.get('op') in ('maelstrom_points','maelstrom_command_points') for op in request.get('operations', [])):
            from .maelstrom_probe import validate
            for op in request['operations']:
                if op.get('op') in ('maelstrom_points','maelstrom_command_points'):
                    validate(op)
                if op.get('op') == 'maelstrom_command_points' and (self.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1):
                    raise OracleProtocolError('Maelstrom commands require a private scheme and one iteration')
        if any(op.get('op') == 'taper_options_command' for op in request.get('operations', [])):
            from .taper_options_probe import validate
            if self.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise OracleProtocolError('Taper preferences require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'taper_options_command':
                    validate(op)
        if any(op.get('op') == 'taper_geometry_command' for op in request.get('operations', [])):
            from .taper_command_probe import validate
            if self.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise OracleProtocolError('Taper geometry commands require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == 'taper_geometry_command':
                    validate(op)
        if any(op.get('op') in ('taper_points', 'taper_command_points') for op in request.get('operations', [])):
            from .taper_probe import validate
            for op in request['operations']:
                if op.get('op') in ('taper_points', 'taper_command_points'):
                    validate(op)
                if op.get('op') == 'taper_command_points':
                    if self.settings_scheme is None:
                        raise OracleProtocolError('Taper commands require a private Rhino settings scheme')
                    if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                        raise OracleProtocolError('Taper commands require one iteration')
        for family in ("twist_points", "twist_command"):
            cases=[op for op in request.get("operations",[]) if op.get("op")==family]
            if cases:
                from . import twist_probe, twist_command_probe
                if family=="twist_command" and self.settings_scheme is None:
                    raise OracleProtocolError("Twist commands require a private Rhino settings scheme")
                if family=="twist_command" and (type(request.get("iterations",1)) is not int or request.get("iterations",1)!=1):
                    raise OracleProtocolError("Twist commands require one iteration")
                for op in cases:
                    (twist_probe if family=="twist_points" else twist_command_probe).validate(op)
        if any(op.get('op') in ('bend_points', 'bend_command_points') for op in request.get('operations', [])):
            from .bend_probe import validate
            for op in request['operations']:
                if op.get('op') in ('bend_points', 'bend_command_points'):
                    validate(op)
                if op.get('op') == 'bend_command_points':
                    if self.settings_scheme is None:
                        raise OracleProtocolError('Bend commands require a private Rhino settings scheme')
                    if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                        raise OracleProtocolError('Bend commands require one iteration')
        if any(op.get('op') == 'twist_options_command' for op in request.get('operations', [])):
            from .twist_options_probe import validate
            if self.settings_scheme is None:
                raise OracleProtocolError('Twist preferences require a private Rhino settings scheme')
            if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise OracleProtocolError('Twist preferences require one iteration')
            for op in request['operations']:
                if op.get('op') == 'twist_options_command':
                    validate(op)
        if any(op.get('op') == 'bend_geometry_command' for op in request.get('operations', [])):
            from .bend_command_probe import validate
            if self.settings_scheme is None:
                raise OracleProtocolError('Bend geometry commands require a private Rhino settings scheme')
            if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise OracleProtocolError('Bend geometry commands require one iteration')
            for op in request['operations']:
                if op.get('op') == 'bend_geometry_command':
                    validate(op)
        if any(op.get('op') == 'bend_options_command' for op in request.get('operations', [])):
            from .bend_options_probe import validate
            if self.settings_scheme is None:
                raise OracleProtocolError('Bend preferences require a private Rhino settings scheme')
            if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                raise OracleProtocolError('Bend preferences require one iteration')
            for op in request['operations']:
                if op.get('op') == 'bend_options_command':
                    validate(op)
        interaction = None
        if any(op.get('op') == 'surface_retrim_profile' for op in request.get('operations', [])):
            from .surface_retrim_profile_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Retrim profiling requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_singular_trim' for op in request.get('operations', [])):
            from .surface_rebuild_singular_trim_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_crossing_hole' for op in request.get('operations', [])):
            from .surface_rebuild_crossing_hole_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_seam_trim' for op in request.get('operations', [])):
            from .surface_rebuild_seam_trim_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_closed_degrees' for op in request.get('operations', [])):
            from .surface_rebuild_closed_degrees_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_closed' for op in request.get('operations', [])):
            from .surface_rebuild_closed_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_natural' for op in request.get('operations', [])):
            from .surface_rebuild_natural_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_retrim_followup' for op in request.get('operations', [])):
            from .surface_rebuild_retrim_followup_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_retrim' for op in request.get('operations', [])):
            from .surface_rebuild_retrim_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_option_followup' for op in request.get('operations', [])):
            from .surface_rebuild_option_followup_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild_options' for op in request.get('operations', [])):
            from .surface_rebuild_options_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_rebuild' for op in request.get('operations', [])):
            from .surface_rebuild_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_control_followup' for op in request.get('operations', [])):
            from .tween_surfaces_control_followup_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('TweenSurfaces requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_control_rows' for op in request.get('operations', [])):
            from .tween_surfaces_control_rows_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('TweenSurfaces requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_control' for op in request.get('operations', [])):
            from .tween_surfaces_control_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('TweenSurfaces requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_corner_sequences' for op in request.get('operations', [])):
            from .tween_surfaces_corner_sequences_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
            from .tween_surfaces_corner_sequence_input import SequencePicker
            interaction=SequencePicker(request)
        if any(op.get('op') == 'tween_surfaces_corners' for op in request.get('operations', [])):
            from .tween_surfaces_corners_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
            from .tween_surfaces_corner_input import CornerPicker
            interaction=CornerPicker(request)
        if any(op.get('op') == 'tween_surfaces_sample_memory' for op in request.get('operations', [])):
            from .tween_surfaces_sample_memory_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_options' for op in request.get('operations', [])):
            from .tween_surfaces_options_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_interaction' for op in request.get('operations', [])):
            from .tween_surfaces_interaction_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_refit' for op in request.get('operations', [])):
            from .tween_surfaces_refit_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_sampling' for op in request.get('operations', [])):
            from .tween_surfaces_sampling_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'tween_surfaces_command' for op in request.get('operations', [])):
            from .tween_surfaces_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'planar_circle_scale' for op in request.get('operations', [])):
            from .planar_circle_scale_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'planar_boolean_scale' for op in request.get('operations', [])):
            from .planar_boolean_scale_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'planar_boolean_mixed' for op in request.get('operations', [])):
            from .planar_boolean_mixed_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'planar_boolean_circular' for op in request.get('operations', [])):
            from .planar_boolean_circular_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'planar_boolean_topology' for op in request.get('operations', [])):
            from .planar_boolean_topology_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'planar_boolean_command' for op in request.get('operations', [])):
            from .planar_boolean_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Planar Boolean requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_two_coplanar' for op in request.get('operations', [])):
            from .boolean_two_coplanar_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Boolean2Objects requires a private settings scheme')
            validate_request(request)
            from .boolean_two_coplanar_input import BooleanTwoCoplanarPicker
            interaction=BooleanTwoCoplanarPicker(request)
        if any(op.get('op') == 'boolean_two_open' for op in request.get('operations', [])):
            from .boolean_two_open_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Boolean2Objects requires a private settings scheme')
            validate_request(request)
            from .boolean_two_open_input import BooleanTwoOpenPicker
            interaction=BooleanTwoOpenPicker(request)
        if any(op.get('op') == 'boolean_two_command' for op in request.get('operations', [])):
            from .boolean_two_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Boolean2Objects requires a private settings scheme')
            validate_request(request)
            from .boolean_two_input import BooleanTwoPicker
            interaction=BooleanTwoPicker(request)
        if any(op.get('op') == 'boolean_split_topology' for op in request.get('operations', [])):
            from .boolean_split_topology_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanSplit topology requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_split_mixed_open' for op in request.get('operations', [])):
            from .boolean_split_mixed_open_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanSplit mixed open targets require a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_split_open' for op in request.get('operations', [])):
            from .boolean_split_open_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanSplit open targets require a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_split_plane' for op in request.get('operations', [])):
            from .boolean_split_plane_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanSplit plane probes require a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_split_command' for op in request.get('operations', [])):
            from .boolean_split_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanSplit requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_difference_order_command' for op in request.get('operations', [])):
            from .boolean_difference_order_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanDifference ordering requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_difference_command' for op in request.get('operations', [])):
            from .boolean_difference_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanDifference commands require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_intersection_command' for op in request.get('operations', [])):
            from .boolean_intersection_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanIntersection commands require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'boolean_union_command' for op in request.get('operations', [])):
            from .boolean_union_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('BooleanUnion commands require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'convex_boolean' for op in request.get('operations', [])):
            from .convex_boolean_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Convex Boolean probes require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'polyhedral_boolean' for op in request.get('operations', [])):
            from .polyhedral_boolean_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Polyhedral Boolean probes require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'compound_intersection' for op in request.get('operations', [])):
            from .compound_intersection_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Compound intersections require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'common_participation' for op in request.get('operations', [])):
            from .common_participation_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Common participation requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_curve_image' for op in request.get('operations', [])):
            from .surface_curve_image_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Surface curve image requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'apply_uv_curves_command' for op in request.get('operations', [])):
            from .apply_uv_curves_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('ApplyCrv requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'create_uv_curves_command' for op in request.get('operations', [])):
            from .create_uv_curves_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('CreateUVCrv requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'uv_face_reference_command' for op in request.get('operations', [])):
            from .uv_face_reference_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('UV face references require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'uv_subcurve_input_command' for op in request.get('operations', [])):
            from .uv_subcurve_input_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('UV SubCrv requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'subcurve_numeric_followup' for op in request.get('operations', [])):
            from .subcurve_numeric_followup_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('numeric followup requires a private Rhino settings scheme')
            validate_request(request)
            from .group_picking import IdlePicker
            interaction=IdlePicker()
        if any(op.get('op') == 'standalone_subcurve' for op in request.get('operations', [])):
            from .standalone_subcurve_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('standalone SubCrv requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'subcurve_mark_ends' for op in request.get('operations', [])):
            from .subcurve_mark_ends_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('MarkEnds requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'subcurve_midpoint' for op in request.get('operations', [])):
            from .subcurve_midpoint_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('FromMidpoint requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'subcurve_preferences' for op in request.get('operations', [])):
            from .subcurve_preferences_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('SubCrv preferences require a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'subcurve_direction' for op in request.get('operations', [])):
            from .subcurve_direction_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('direction locking requires a private settings scheme')
            validate_request(request)
            from .subcurve_direction_input import SubcurveDirectionPicker
            interaction=SubcurveDirectionPicker(request)
        if any(op.get('op') == 'subcurve_direction_grid' for op in request.get('operations', [])):
            from .subcurve_direction_grid_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('direction matrix requires a private settings scheme')
            validate_request(request)
            from .subcurve_direction_grid_input import DirectionGridPicker
            interaction=DirectionGridPicker(request)
        if any(op.get('op') == 'subcurve_edge' for op in request.get('operations', [])):
            from .subcurve_edge_probe import validate_request
            if self.settings_scheme is None:raise OracleProtocolError('edge SubCrv requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_pullback_endpoints' for op in request.get('operations', [])):
            from .surface_pullback_endpoints_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Pullback endpoints requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_pullback_linear' for op in request.get('operations', [])):
            from .surface_pullback_linear_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Linear pullback requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'surface_pullback_interpolation' for op in request.get('operations', [])):
            from .surface_pullback_interpolation_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Interpolated pullback requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'compound_pairs' for op in request.get('operations', [])):
            from .compound_pairs_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Compound pairs require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'polyhedral_boolean_command' for op in request.get('operations', [])):
            from .polyhedral_command_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Polyhedral commands require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'grip_alias' for op in request.get('operations', [])):
            from .grip_alias_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Grip aliases require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'smooth_workflow' for op in request.get('operations', [])):
            from .smooth_workflow_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Smooth workflows require a private Rhino settings scheme')
            validate_request(request)
            from .smooth_workflow_input import SmoothKeyboard
            interaction = SmoothKeyboard(request)
        if any(op.get('op') == 'smooth_uvn' for op in request.get('operations', [])):
            from .smooth_uvn_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Smooth UVN requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'smooth_frames' for op in request.get('operations', [])):
            from .smooth_frames_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Smooth direction witnesses require a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'smooth_command' for op in request.get('operations', [])):
            from .smooth_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Smooth requires a private Rhino settings scheme')
            validate_request(request)
        if any(op.get('op') == 'scale_by_plane_curve' for op in request.get('operations', [])):
            from .scale_by_plane_curve_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('ScaleByPlane curve frames require a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'scale_by_plane_object' for op in request.get('operations', [])):
            from .scale_by_plane_object_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('ScaleByPlane Object requires a private settings scheme')
            validate_request(request)
            from .scale_by_plane_object_input import ScaleByPlaneObjectPicker
            interaction = ScaleByPlaneObjectPicker(request)
        if any(op.get('op') == 'scale_by_plane' for op in request.get('operations', [])):
            from .scale_by_plane_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('ScaleByPlane requires a private settings scheme')
            validate_request(request)
            from .scale_by_plane_input import ScaleByPlaneViewPicker
            interaction = ScaleByPlaneViewPicker(request)
        if any(op.get('op') == 'point_input_precision' for op in request.get('operations', [])):
            from .point_input_precision_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('Point input precision requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'scale_positions_cursor' for op in request.get('operations', [])):
            from .scale_positions_cursor_input import ScalePositionsCursorPicker
            if self.settings_scheme is None:
                raise OracleProtocolError('ScalePositions cursor requires a private settings scheme')
            interaction = ScalePositionsCursorPicker(request)
        if any(op.get('op') == 'scale_positions' for op in request.get('operations', [])):
            from .scale_positions_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('ScalePositions requires a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'scale_nu_options' for op in request.get('operations', [])):
            from .scale_nu_options_probe import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError('ScaleNU options require a private settings scheme')
            validate_request(request)
        if any(op.get('op') == 'scale_nu_reference' for op in request.get('operations', [])):
            from .scale_nu_reference_input import ScaleNuReferencePicker
            if self.settings_scheme is None:
                raise OracleProtocolError('ScaleNU references require a private settings scheme')
            interaction = ScaleNuReferencePicker(request)
        if any(op.get('op') == 'maelstrom_preview' for op in request.get('operations', [])):
            from .maelstrom_preview_input import MaelstromPreviewPicker
            if self.settings_scheme is None:
                raise OracleProtocolError('Maelstrom previews require a private settings scheme')
            interaction=MaelstromPreviewPicker(request)
        if any(op.get('op') == 'taper_preview' for op in request.get('operations', [])):
            from .taper_preview_input import TaperPreviewPicker
            if self.settings_scheme is None:
                raise OracleProtocolError('Taper previews require a private settings scheme')
            interaction=TaperPreviewPicker(request)
        if any(op.get('op') == 'bend_preview' for op in request.get('operations', [])):
            from .bend_preview_input import BendPreviewPicker
            if self.settings_scheme is None:
                raise OracleProtocolError('Bend previews require a private settings scheme')
            interaction = BendPreviewPicker(request)
        if any(op.get('op') == 'twist_preview' for op in request.get('operations', [])):
            from .twist_preview_input import TwistPreviewPicker
            if self.settings_scheme is None:
                raise OracleProtocolError('Twist previews require a private settings scheme')
            interaction = TwistPreviewPicker(request)
        if any(op.get('op') == 'mirror_preview' for op in request.get('operations', [])):
            from .mirror_preview_input import MirrorPreviewPicker
            if self.settings_scheme is None:
                raise OracleProtocolError('Mirror previews require a private Rhino settings scheme')
            interaction = MirrorPreviewPicker(request)
        if any(op.get('op') == 'translation_preview' for op in request.get('operations',[])):
            from .translation_preview_input import TranslationPreviewPicker
            if self.settings_scheme is None:raise OracleProtocolError('translation previews require a private scheme')
            interaction=TranslationPreviewPicker(request)
        if any(op.get('op') == 'affine_preview' for op in request.get('operations',[])):
            from .affine_preview_input import AffinePreviewPicker
            if self.settings_scheme is None:raise OracleProtocolError('affine previews require a private scheme')
            interaction=AffinePreviewPicker(request)
        if any(op.get("op") == "transform_copy_command" for op in request.get("operations", [])):
            from .transform_copy_capture import validate_request
            if self.settings_scheme is None:
                raise OracleProtocolError("transform Copy probes require a private Rhino settings scheme")
            validate_request(request)
            if any('normal_target' in op or 'mouse_target' in op or op.get('mirror_target', {}).get('pick') in ('mouse', 'mouse-sub') for op in request['operations']):
                from .component_picking import ComponentPicker
                interaction = ComponentPicker()

        if any(op.get("op") == "copy_options_command" for op in request.get("operations", [])):
            from .copy_options_probe import validate
            if self.settings_scheme is None:
                raise OracleProtocolError("Copy settings probes require a private Rhino settings scheme")
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("Copy settings probes require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "copy_options_command": validate(operation)
        if any(op.get("op") == "brep_remove_holes" for op in request.get("operations", [])):
            from .remove_holes_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("hole removal requires one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "brep_remove_holes": validate(operation)
        if any(op.get("op") == "brep_unjoin_edges" for op in request.get("operations", [])):
            from .unjoin_edges_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("edge separation requires one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "brep_unjoin_edges": validate(operation)
        if any(op.get("op") == "unjoin_edge_command" for op in request.get("operations", [])):
            from .unjoin_edge_command_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("edge separation commands require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "unjoin_edge_command": validate(operation)
            if any(op.get("pick") in ("mouse", "sequence") for op in request["operations"]):
                from .component_picking import ComponentPicker
                interaction = ComponentPicker()
        if any(op.get("op") == "extract_srf_command" for op in request.get("operations", [])):
            from .extract_srf_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("ExtractSrf commands require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "extract_srf_command": validate(operation)
            if any(op.get("op") == "extract_srf_command" and op.get("pick") == "sequence" for op in request["operations"]):
                from .component_picking import ComponentPicker
                interaction = ComponentPicker()
        if any(op.get("op") in ("shrink_trimmed_srf_command", "shrink_trimmed_srf_to_edge_command") for op in request.get("operations", [])):
            from .shrink_trimmed_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("shrink commands require one iteration")
            for operation in request["operations"]:
                if operation.get("op") in ("shrink_trimmed_srf_command", "shrink_trimmed_srf_to_edge_command"): validate(operation)
            if any(op.get("pick") == "sequence" for op in request["operations"]):
                from .component_picking import ComponentPicker
                interaction = ComponentPicker()
        if any(op.get("op") == "untrim_command" for op in request.get("operations", [])):
            from .untrim_component_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("Untrim commands require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "untrim_command": validate(operation)
            if any(op.get("pick") == "mouse" for op in request["operations"]):
                from .component_picking import ComponentPicker
                interaction = ComponentPicker()
        if any(op.get("op") in ("document_brep", "document_brep_import") for op in request.get("operations", [])):
            from .document_brep_probe import validate
            for operation in request["operations"]:
                if operation.get("op") in ("document_brep", "document_brep_import"): validate(operation, request.get("iterations", 1))
        if any(op.get("op") == "orientation_audit" for op in request.get("operations", [])):
            from .orientation_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("orientation audit requires one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "orientation_audit": validate(operation)
        if any(op.get("op") in ("untrim_all_command", "untrim_border_command") for op in request.get("operations", [])):
            from .untrim_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("untrim commands require one iteration")
            for operation in request["operations"]:
                if operation.get("op") in ("untrim_all_command", "untrim_border_command"): validate(operation)
        if any(op.get("op") == "untrim_holes_command" for op in request.get("operations", [])):
            from .untrim_holes_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("hole commands require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "untrim_holes_command": validate(operation)
            if any(op.get("pick") in ("mouse", "window") for op in request["operations"]):
                # The shared picker also accepts sequenced UnjoinEdge inputs
                # when a protocol request mixes these component commands.
                from .component_picking import ComponentPicker
                interaction = ComponentPicker()
        if any(op.get("op") == "view_camera_probe" for op in request.get("operations", [])):
            from .view_camera_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("camera probes require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "view_camera_probe":
                    validate(operation)
        if any(op.get("op") == "set_view_prompt_probe" for op in request.get("operations", [])):
            from .set_view_prompt_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("SetView prompt probes require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "set_view_prompt_probe":
                    validate(operation)
        if any(op.get("op") == "named_view_policy_probe" for op in request.get("operations", [])):
            from .named_view_policy_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("named-view policy probes require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "named_view_policy_probe":
                    validate(operation)
        if any(op.get("op") == "zoom_extents_probe" for op in request.get("operations", [])):
            from .zoom_extents_probe import validate
            if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
                raise OracleProtocolError("Zoom fitting probes require one iteration")
            for operation in request["operations"]:
                if operation.get("op") == "zoom_extents_probe":
                    validate(operation)
        if any(op.get("op") in ("synchronize_cplanes_probe", "copy_cplane_probe")
               for op in request.get("operations", [])):
            from .group_picking import IdlePicker
            interaction = IdlePicker()
        if any(op.get("op") == "view_camera_probe" and "mouse_drag" in op
               for op in request.get("operations", [])):
            from .camera_navigation import CameraNavigator
            interaction = CameraNavigator(request)
        if any(op.get("op") in ("area_centroid_command", "volume_centroid_command", "volume_command") for op in request.get("operations", [])):
            from .area_centroid_probe import validate
            if type(request.get("iterations",1)) is not int or request.get("iterations",1) != 1:
                raise OracleProtocolError("centroid commands require one iteration")
            for operation in request["operations"]:
                if operation.get("op") in ("area_centroid_command", "volume_centroid_command", "volume_command"):
                    validate(operation, operation["op"].split("_")[0], operation["op"] != "volume_command")
        if any("open_confirmation" in op for op in request.get("operations", [])):
            from .volume_confirmation import VolumeConfirmation
            interaction = VolumeConfirmation(request)
        if any(op.get("op") == "point_snap" for op in request.get("operations", [])):
            from .point_snap_input import PointSnapPicker
            interaction = PointSnapPicker(request)
        if any(op.get("op") == "angle_cursor_diagnostic" or
               (op.get("op") == "plane_primitive" and op.get("primitive") in
                ("ArcCenterEndpoint", "ArcStartCenterEndpoint", "ArcMidpointEndpoint", "ArcDefaultEndpoint"))
               for op in request.get("operations", [])):
            from .group_picking import IdlePicker
            if any(op.get("op") == "angle_cursor_diagnostic" for op in request.get("operations", [])):
                from .angle_cursor_probe import validate_request
                validate_request(request)
            class AnglePicker(IdlePicker):
                def send_input(self, name, x, y, window):
                    # The worker announces the target before RunScript reaches _Pause.
                    time.sleep(1.0)
                    subprocess.run(["xdotool", "windowactivate", "--sync", window,
                                    "mousemove", str(int(x) + 1), y,
                                    "mousemove", x, y], check=True, timeout=10)
                    time.sleep(0.5)
                    subprocess.run(["xdotool", "windowactivate", "--sync", window,
                                    "click", "1"], check=True, timeout=10)
                    return True
            interaction = AnglePicker()
        if any(op.get("op") in ("merge_edge_command", "split_edge_command") and op.get("pick") == "mouse"
               for op in request.get("operations", [])):
            from .group_picking import IdlePicker
            from .merge_edges_probe import validate_mouse_request
            validate_mouse_request(request)
            interaction = IdlePicker()
        if any(op.get("op") in ("mesh_connected_command_probe", "mesh_part_command_probe")
               and op.get("mouse_pick")
               for op in request.get("operations", [])):
            from .group_picking import IdlePicker
            accept = []
            pause = []
            for op in request["operations"]:
                if op.get("op") != "mesh_part_command_probe":
                    continue
                count = len(op.get("pick_points", [op.get("pick_point")]))
                names = ([op["id"]] if count == 1 else
                         ["%s-%d" % (op["id"], index + 1) for index in range(count)])
                pause.extend(names)
                accept.append(names[-1])
            interaction = IdlePicker(accept_after_click=accept,
                                     pause_after_click=pause)
        if any(operation.get("op") in ("group_picking", "mesh_split_picking", "mesh_explode_picking") for operation in request.get("operations", [])):
            from .group_picking import IdlePicker, validate_request
            validate_request(request)
            worker_source = Path(__file__).with_name("group_picking_worker.py")
            interaction = IdlePicker()
        if any(operation.get("op") == "undo_selection" for operation in request.get("operations", [])):
            from .undo_selection import validate_request
            validate_request(request)
            worker_source = Path(__file__).with_name("undo_selection_worker.py")
        if not worker_source.is_file():
            raise OracleError(f"Rhino worker not found: {worker_source}")

        block_controls = any(op.get('op') == 'block_workflow' and step.get('action') == 'edit_roundtrip' and any(step.get(k) is not None and step.get(k) != [] for k in ('add_objects','remove_members','base_point','contexts')) for op in request['operations'] for step in op.get('steps',[]))
        with tempfile.TemporaryDirectory(prefix="viboceros-rhino-oracle-") as job:
            job_path = Path(job)
            request_path = job_path / "request.json"
            response_path = job_path / "response.json"
            worker_path = job_path / "rhino_worker.py"
            if any(op.get('op') == 'block_workflow' for op in request.get('operations', [])):
                shutil.copyfile(worker_source.with_name('block_workflow_probe.py'), job_path / 'block_workflow_probe.py')
                if block_controls:
                    from .block_edit_controls_input import BlockEditController
                    interaction = BlockEditController(request)
                    shutil.copyfile(worker_source.with_name('block_edit_controls_probe.py'),job_path / 'block_edit_controls_probe.py')
                    shutil.copyfile(worker_source.with_name('block_edit_context_probe.py'),job_path / 'block_edit_context_probe.py')
            if any(op.get('op') == 'mesh_edit_records' for op in request.get('operations', [])):
                shutil.copyfile(worker_source.with_name('mesh_edit_records_probe.py'), job_path / 'mesh_edit_records_probe.py')
            if any(op.get('op') == 'grip_transform' for op in request.get('operations', [])):
                for name in ('grip_transform_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') in ('circle_fit_grips', 'circle_fit_grips_commands') for op in request.get('operations', [])):
                for name in ('circle_fit_grips_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'circle_fit_selection' for op in request.get('operations', [])):
                for name in ('circle_fit_selection_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'maelstrom_fit_points_command' for op in request.get('operations', [])):
                for name in ('maelstrom_fit_points_probe.py', 'number_token.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'circle_fit_diagnostics' for op in request.get('operations', [])):
                for name in ('circle_fit_diagnostics.py', 'circle_fit_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'circle_fit_benchmark' for op in request.get('operations', [])):
                shutil.copyfile(worker_source.with_name('circle_fit_benchmark.py'), job_path / 'circle_fit_benchmark.py')
            if any(op.get('op') == 'circle_fit_points' for op in request.get('operations', [])):
                for name in ('circle_fit_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'scale_nu_reference' for op in request.get('operations', [])):
                for name in ('scale_nu_reference_probe.py', 'join_probe.py', 'merge_edges_probe.py', 'shrink_face_input.py', 'snap_environment.py', 'viewport_capture.py', 'named_view_policy_probe.py', 'view_camera_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'point_input_precision' for op in request.get('operations', [])):
                for name in ('point_input_precision_probe.py','number_token.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            worker_request = dict(request)
            if any(op.get('op') == 'surface_retrim_profile' for op in request.get('operations', [])):
                for name in ('surface_retrim_profile_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'surface_rebuild_singular_trim' for op in request.get('operations', [])):
                for name in ('surface_rebuild_singular_trim_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_crossing_hole' for op in request.get('operations', [])):
                for name in ('surface_rebuild_crossing_hole_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_seam_trim' for op in request.get('operations', [])):
                for name in ('surface_rebuild_seam_trim_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_closed_degrees' for op in request.get('operations', [])):
                for name in ('surface_rebuild_closed_degrees_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_closed' for op in request.get('operations', [])):
                for name in ('surface_rebuild_closed_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_natural' for op in request.get('operations', [])):
                for name in ('surface_rebuild_natural_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_retrim_followup' for op in request.get('operations', [])):
                for name in ('surface_rebuild_retrim_followup_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_retrim' for op in request.get('operations', [])):
                for name in ('surface_rebuild_retrim_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_option_followup' for op in request.get('operations', [])):
                for name in ('surface_rebuild_option_followup_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild_options' for op in request.get('operations', [])):
                for name in ('surface_rebuild_options_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'surface_rebuild' for op in request.get('operations', [])):
                for name in ('surface_rebuild_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_control_followup' for op in request.get('operations', [])):
                for name in ('tween_surfaces_control_followup_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_control_rows' for op in request.get('operations', [])):
                for name in ('tween_surfaces_control_rows_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_control' for op in request.get('operations', [])):
                for name in ('tween_surfaces_control_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_corner_sequences' for op in request.get('operations', [])):
                for name in ('tween_surfaces_corner_sequences_probe.py','join_probe.py','merge_edges_probe.py','shrink_face_input.py','snap_environment.py','viewport_capture.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_corners' for op in request.get('operations', [])):
                for name in ('tween_surfaces_corners_probe.py','join_probe.py','merge_edges_probe.py','shrink_face_input.py','snap_environment.py','viewport_capture.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_sample_memory' for op in request.get('operations', [])):
                for name in ('tween_surfaces_sample_memory_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_options' for op in request.get('operations', [])):
                for name in ('tween_surfaces_options_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_interaction' for op in request.get('operations', [])):
                for name in ('tween_surfaces_interaction_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_refit' for op in request.get('operations', [])):
                for name in ('tween_surfaces_refit_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_sampling' for op in request.get('operations', [])):
                for name in ('tween_surfaces_sampling_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'tween_surfaces_command' for op in request.get('operations', [])):
                for name in ('tween_surfaces_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'planar_circle_scale' for op in request.get('operations', [])):
                for name in ('planar_circle_scale_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'planar_boolean_scale' for op in request.get('operations', [])):
                for name in ('planar_boolean_scale_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'planar_boolean_mixed' for op in request.get('operations', [])):
                for name in ('planar_boolean_mixed_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'planar_boolean_circular' for op in request.get('operations', [])):
                for name in ('planar_boolean_circular_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'planar_boolean_topology' for op in request.get('operations', [])):
                for name in ('planar_boolean_topology_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'planar_boolean_command' for op in request.get('operations', [])):
                for name in ('planar_boolean_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_two_coplanar' for op in request.get('operations', [])):
                for name in ('boolean_two_coplanar_probe.py','boolean_split_open_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_two_open' for op in request.get('operations', [])):
                for name in ('boolean_two_open_probe.py','boolean_split_open_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_two_command' for op in request.get('operations', [])):
                for name in ('boolean_two_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_split_topology' for op in request.get('operations', [])):
                for name in ('boolean_split_topology_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_split_mixed_open' for op in request.get('operations', [])):
                for name in ('boolean_split_mixed_open_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_split_open' for op in request.get('operations', [])):
                for name in ('boolean_split_open_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_split_plane' for op in request.get('operations', [])):
                for name in ('boolean_split_plane_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'boolean_split_command' for op in request.get('operations', [])):
                for name in ('boolean_split_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'boolean_difference_order_command' for op in request.get('operations', [])):
                for name in ('boolean_difference_order_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'boolean_difference_command' for op in request.get('operations', [])):
                for name in ('boolean_difference_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'boolean_intersection_command' for op in request.get('operations', [])):
                for name in ('boolean_intersection_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'boolean_union_command' for op in request.get('operations', [])):
                for name in ('boolean_union_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'convex_boolean' for op in request.get('operations', [])):
                for name in ('convex_boolean_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'polyhedral_boolean' for op in request.get('operations', [])):
                for name in ('polyhedral_boolean_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'compound_intersection' for op in request.get('operations', [])):
                for name in ('compound_intersection_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'common_participation' for op in request.get('operations', [])):
                for name in ('common_participation_probe.py', 'compound_recipe_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'surface_curve_image' for op in request.get('operations', [])):
                for name in ('surface_curve_image_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'surface_pullback_endpoints' for op in request.get('operations', [])):
                for name in ('surface_pullback_endpoints_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'surface_pullback_linear' for op in request.get('operations', [])):
                for name in ('surface_pullback_linear_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'surface_pullback_interpolation' for op in request.get('operations', [])):
                for name in ('surface_pullback_interpolation_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'compound_pairs' for op in request.get('operations', [])):
                for name in ('compound_pairs_probe.py', 'compound_recipe_probe.py', 'compound_intersection_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'polyhedral_boolean_command' for op in request.get('operations', [])):
                for name in ('polyhedral_command_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'apply_uv_curves_command' for op in request.get('operations', [])):
                for name in ('apply_uv_curves_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'create_uv_curves_command' for op in request.get('operations', [])):
                for name in ('create_uv_curves_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'uv_face_reference_command' for op in request.get('operations', [])):
                for name in ('uv_face_reference_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'uv_subcurve_input_command' for op in request.get('operations', [])):
                for name in ('uv_subcurve_input_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path/name)
            if any(op.get('op') == 'subcurve_numeric_followup' for op in request.get('operations', [])):
                for name in ('subcurve_numeric_followup_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'standalone_subcurve' for op in request.get('operations', [])):
                for name in ('standalone_subcurve_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'subcurve_mark_ends' for op in request.get('operations', [])):
                for name in ('subcurve_mark_ends_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'subcurve_midpoint' for op in request.get('operations', [])):
                for name in ('subcurve_midpoint_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'subcurve_preferences' for op in request.get('operations', [])):
                for name in ('subcurve_preferences_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'subcurve_direction' for op in request.get('operations', [])):
                for name in ('subcurve_direction_probe.py','join_probe.py','merge_edges_probe.py','shrink_face_input.py','snap_environment.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'subcurve_direction_grid' for op in request.get('operations', [])):
                for name in ('subcurve_direction_grid_probe.py','join_probe.py','merge_edges_probe.py','shrink_face_input.py','snap_environment.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'subcurve_edge' for op in request.get('operations', [])):
                for name in ('subcurve_edge_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'curve_subcurve_arc_length' for op in request.get('operations', [])):
                shutil.copyfile(worker_source.with_name('curve_length_subcurve_probe.py'),job_path/'curve_length_subcurve_probe.py')
            if any(op.get('op') == 'grip_alias' for op in request.get('operations', [])):
                for name in ('grip_alias_probe.py','smooth_probe.py','join_probe.py','merge_edges_probe.py','grip_transform_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'smooth_workflow' for op in request.get('operations', [])):
                for name in ('smooth_workflow_probe.py','smooth_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'smooth_uvn' for op in request.get('operations', [])):
                for name in ('smooth_uvn_probe.py','smooth_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'smooth_frames' for op in request.get('operations', [])):
                for name in ('smooth_frames_probe.py','smooth_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'smooth_command' for op in request.get('operations', [])):
                for name in ('smooth_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'scale_by_plane_curve' for op in request.get('operations', [])):
                for name in ('scale_by_plane_curve_probe.py','join_probe.py','merge_edges_probe.py','number_token.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'scale_by_plane_object' for op in request.get('operations', [])):
                for name in ('scale_by_plane_object_probe.py','join_probe.py','merge_edges_probe.py','number_token.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'scale_by_plane' for op in request.get('operations', [])):
                for name in ('scale_by_plane_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py','number_token.py','shrink_face_input.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'scale_positions' for op in request.get('operations', [])):
                for name in ('scale_positions_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'scale_positions_cursor' for op in request.get('operations', [])):
                for name in ('scale_positions_cursor_probe.py','number_token.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py','shrink_face_input.py',
                             'snap_environment.py','viewport_capture.py','named_view_policy_probe.py','view_camera_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'scale_nu_options' for op in request.get('operations', [])):
                for name in ('scale_nu_options_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'scale_nu' for op in request.get('operations', [])):
                for name in ('scale_nu_probe.py', 'grip_transform_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            worker_request["_host"] = {"exit_rhino_when_complete": True}
            _write_json(request_path, worker_request)
            if block_controls:
                shutil.copyfile(worker_source, job_path / 'block_edit_idle_core.py')
                worker_path.write_text("""# -*- coding: utf-8 -*-
import Rhino
import block_edit_idle_core as core

def execute_owned_controls(sender, args):
    Rhino.RhinoApp.Idle -= execute_owned_controls
    core._main()

core._record_progress('block edit controls scheduled on idle')
Rhino.RhinoApp.Idle += execute_owned_controls
""", encoding='utf-8')
            else:
                shutil.copyfile(worker_source, worker_path)
            if any(op.get("op") == "pipe_round_probe" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("pipe_round_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "polygon_count_probe" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("polygon_count_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "self_intersect_probe" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("self_intersect_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "mesh_offset_probe" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("mesh_offset_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") in ("document_brep", "document_brep_import") for op in request.get("operations", [])):
                helper = Path(__file__).with_name("document_brep_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "orientation_audit" for op in request.get("operations", [])):
                for name in ("orientation_probe.py", "join_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") in ("untrim_all_command", "untrim_border_command") for op in request.get("operations", [])):
                for name in ("untrim_probe.py", "join_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "untrim_holes_command" for op in request.get("operations", [])):
                for name in ("untrim_holes_probe.py", "join_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "view_camera_probe" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("view_camera_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "set_view_prompt_probe" for op in request.get("operations", [])):
                for name in ("set_view_prompt_probe.py", "view_camera_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "named_view_policy_probe" for op in request.get("operations", [])):
                for name in ("named_view_policy_probe.py", "view_camera_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "zoom_extents_probe" for op in request.get("operations", [])):
                for name in ("zoom_extents_probe.py", "named_view_policy_probe.py", "view_camera_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") in ("area_centroid_command", "volume_centroid_command", "volume_command") for op in request.get("operations", [])):
                helper = Path(__file__).with_name("area_centroid_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") in ("point_snap", "split_edge_command") for op in request.get("operations", [])):
                helper = Path(__file__).with_name("snap_environment.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "point_snap" for op in request.get("operations", [])):
                for name in ("point_snap_probe.py", "viewport_capture.py", "named_view_policy_probe.py", "view_camera_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "angle_cursor_diagnostic" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("angle_cursor_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get('op') == 'mirror_preview' for op in request.get('operations', [])):
                helper = Path(__file__).with_name('mirror_preview_probe.py')
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get('op') == 'translation_preview' for op in request.get('operations',[])):
                for name in ('translation_preview_probe.py','viewport_capture.py','named_view_policy_probe.py','view_camera_probe.py','shrink_face_input.py','snap_environment.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') in ('twist_command', 'twist_options_command') for op in request.get('operations',[])):
                for name in ('twist_command_probe.py','join_probe.py','merge_edges_probe.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'twist_options_command' for op in request.get('operations', [])):
                helper=Path(__file__).with_name('twist_options_probe.py');shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'twist_points' for op in request.get('operations',[])):
                helper=Path(__file__).with_name('twist_probe.py');shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') in ('bend_points', 'bend_command_points') for op in request.get('operations', [])):
                for name in ('bend_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') in ('taper_points', 'taper_command_points') for op in request.get('operations', [])):
                for name in ('taper_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') in ('maelstrom_points','maelstrom_command_points') for op in request.get('operations', [])):
                for name in ('maelstrom_probe.py','number_token.py','join_probe.py','merge_edges_probe.py'):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get('op') == 'taper_options_command' for op in request.get('operations', [])):
                for name in ('taper_options_probe.py', 'twist_command_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') in ('maelstrom_geometry_command','maelstrom_options_command','maelstrom_input_command','maelstrom_circle_command') for op in request.get('operations', [])):
                for name in ('maelstrom_command_probe.py','maelstrom_options_probe.py','maelstrom_input_probe.py','maelstrom_circle_probe.py','number_token.py','twist_command_probe.py','join_probe.py','merge_edges_probe.py'):
                    shutil.copyfile(worker_source.with_name(name), job_path / name)
            if any(op.get('op') == 'taper_geometry_command' for op in request.get('operations', [])):
                for name in ('taper_command_probe.py', 'number_token.py', 'twist_command_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'bend_geometry_command' for op in request.get('operations', [])):
                for name in ('bend_command_probe.py', 'twist_command_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'bend_options_command' for op in request.get('operations', [])):
                for name in ('bend_options_probe.py', 'twist_command_probe.py', 'join_probe.py', 'merge_edges_probe.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'twist_preview' for op in request.get('operations',[])):
                for name in ('twist_preview_probe.py','twist_command_probe.py','viewport_capture.py','named_view_policy_probe.py','view_camera_probe.py','shrink_face_input.py','snap_environment.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'maelstrom_preview' for op in request.get('operations', [])):
                for name in ('maelstrom_preview_probe.py','number_token.py','twist_command_probe.py','viewport_capture.py','named_view_policy_probe.py','view_camera_probe.py','shrink_face_input.py','snap_environment.py'):
                    shutil.copyfile(worker_source.with_name(name),job_path/name)
            if any(op.get('op') == 'taper_preview' for op in request.get('operations', [])):
                for name in ('taper_preview_probe.py','number_token.py','twist_command_probe.py','viewport_capture.py','named_view_policy_probe.py','view_camera_probe.py','shrink_face_input.py','snap_environment.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'bend_preview' for op in request.get('operations', [])):
                for name in ('bend_preview_probe.py', 'twist_command_probe.py', 'join_probe.py', 'merge_edges_probe.py', 'viewport_capture.py', 'named_view_policy_probe.py', 'view_camera_probe.py', 'shrink_face_input.py', 'snap_environment.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get('op') == 'affine_preview' for op in request.get('operations',[])):
                for name in ('affine_preview_probe.py','viewport_capture.py','named_view_policy_probe.py','view_camera_probe.py','shrink_face_input.py','snap_environment.py'):
                    helper=Path(__file__).with_name(name);shutil.copyfile(helper,job_path/helper.name)
            if any(op.get("op") == "mesh_snap_settings" or "snap_to_meshes" in op for op in request.get("operations", [])):
                helper = Path(__file__).with_name("mesh_snap_settings_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "border_command" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("border_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "mesh_cap_command" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("mesh_cap_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "mesh_draft_angle_command" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("mesh_draft_angle_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") in ("join_command", "cap_command", "merge_edges_command", "merge_edge_command", "split_edge_command", "area_centroid_command", "volume_centroid_command", "volume_command") for op in request.get("operations", [])):
                helper = Path(__file__).with_name("join_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            brep_join_commands = any(op.get("op") == "join_command" and any("brep" in s for s in op.get("sources", [])) for op in request.get("operations", []))
            if brep_join_commands or any(op.get("op") in ("cap_command", "brep_join", "merge_edges_command", "merge_edge_command", "split_edge_command", "brep_merge_edge") for op in request.get("operations", [])):
                helper = Path(__file__).with_name("cap_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if brep_join_commands or any(op.get("op") in ("brep_join", "merge_edges_command", "merge_edge_command", "split_edge_command", "brep_merge_edge") for op in request.get("operations", [])):
                helper = Path(__file__).with_name("brep_join_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") in ("merge_edges_command", "merge_edge_command", "split_edge_command") or
                   (op.get("op") in ("untrim_holes_command", "unjoin_edge_command", "untrim_command", "shrink_trimmed_srf_command", "shrink_trimmed_srf_to_edge_command", "extract_srf_command") and op.get("undo_redo", False))
                   for op in request.get("operations", [])):
                helper = Path(__file__).with_name("merge_edges_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "split_edge_command" for op in request.get("operations", [])):
                for name in ("split_edge_probe.py", "viewport_capture.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "brep_merge_edge" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("merge_edge_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "brep_remove_holes" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("remove_holes_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "brep_unjoin_edges" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("unjoin_edges_probe.py")
                shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "unjoin_edge_command" for op in request.get("operations", [])):
                for name in ("unjoin_edge_command_probe.py", "join_probe.py", "untrim_holes_probe.py", "unjoin_edge_input.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "extract_srf_command" for op in request.get("operations", [])):
                for name in ("extract_srf_probe.py", "owned_brep_command.py", "join_probe.py", "shrink_face_input.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "transform_copy_command" for op in request.get("operations", [])):
                for name in ("transform_copy_probe.py", "move_normal_probe.py", "translation_input.py", "viewport_capture.py", "named_view_policy_probe.py", "view_camera_probe.py", "snap_environment.py", "mirror_object_probe.py", "shrink_face_input.py", "join_probe.py", "merge_edges_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)

            if any(op.get("op") == "copy_options_command" for op in request.get("operations", [])):
                for name in ("copy_options_probe.py", "owned_brep_command.py", "join_probe.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") in ("shrink_trimmed_srf_command", "shrink_trimmed_srf_to_edge_command") for op in request.get("operations", [])):
                for name in ("shrink_trimmed_probe.py", "owned_brep_command.py", "join_probe.py", "shrink_face_input.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "untrim_command" for op in request.get("operations", [])):
                for name in ("untrim_component_probe.py", "owned_brep_command.py", "join_probe.py", "component_command_input.py"):
                    helper = Path(__file__).with_name(name)
                    shutil.copyfile(helper, job_path / helper.name)
            if any(op.get("op") == "document_units" for op in request.get("operations", [])):
                helper = Path(__file__).with_name("generate_document_units_reference.py")
                shutil.copyfile(helper, job_path / helper.name)
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
            if self.settings_scheme is not None:
                command.insert(3, "/scheme=" + self.settings_scheme)
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

            started_waiting = time.monotonic()
            deadline = started_waiting + timeout
            startup_deadline = started_waiting + min(timeout, RHINO_STARTUP_GRACE_SECONDS)
            procfs_available = Path("/proc").is_dir()
            owned_pids: set[int] = set()
            owned_window: str | None = None
            fallback_ready_at: float | None = None
            fallback_sent = False
            worker_exited = False
            startup_failed = False
            process_missing_since: float | None = None
            try:
                while not response_path.is_file() and time.monotonic() < deadline:
                    owned_pids.update(
                        _rhino_process_ids(windows_worker) - existing_pids
                    )
                    if _owned_worker_exited(owned_pids, job_path / "worker-progress.log"):
                        worker_exited = True
                        break
                    if (owned_pids and procfs_available
                            and _wait_for_process_exit(owned_pids, 0.0)):
                        if process_missing_since is None:
                            process_missing_since = time.monotonic()
                        elif time.monotonic() - process_missing_since >= RHINO_PROCESS_EXIT_GRACE_SECONDS:
                            worker_exited = True
                            break
                    else:
                        process_missing_since = None
                    if (procfs_available and not owned_pids
                            and time.monotonic() >= startup_deadline):
                        startup_failed = True
                        break
                    if (not fallback_sent and _ui_fallback_enabled()
                            and not _read_optional_text(job_path / "worker-progress.log")):
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
                    if interaction is not None:
                        interaction(job_path, owned_pids)
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
                    if worker_exited:
                        failure = "Rhino process exited before publishing a response"
                    elif startup_failed:
                        failure = "Rhino process never appeared after launcher completion"
                    else:
                        failure = f"Rhino probe did not respond within {timeout:g} seconds"
                    raise OracleError(
                        f"{failure} "
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
        _validate_response(response, "rhino", request)
        if any(op.get("op") == "scale_by_plane" for op in request.get("operations", [])):
            interaction.record_diagnostics(response)
        if any(op.get('op') in ('mirror_preview','translation_preview','affine_preview','twist_preview','bend_preview','taper_preview','maelstrom_preview','scale_nu_reference') for op in request.get('operations', [])):
            interaction.record_diagnostics(response)
        if any(op.get("op") == "point_snap" for op in request.get("operations", [])):
            interaction.record_diagnostics(response)
        if any("open_confirmation" in op for op in request.get("operations", [])):
            interaction.record_diagnostics(response)
        if any(op.get('op') == 'smooth_workflow' for op in request.get('operations', [])):
            interaction.record_diagnostics(response)
        if block_controls:
            interaction.record_diagnostics(response)
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
        with _owned_artifact_request(request) as prepared:
            viboceros = self.run_viboceros(prepared, timeout)
            rhino = self.run_rhino(prepared, timeout)
        return compare_responses(
            viboceros,
            rhino,
            absolute_epsilon=absolute_epsilon,
            relative_epsilon=relative_epsilon,
        )


@contextmanager
def _owned_artifact_request(request):
    """Keep comparison/replay exports off caller paths, with scoped cleanup."""
    with tempfile.TemporaryDirectory(prefix="viboceros-interchange-") as job:
        prepared = copy.deepcopy(dict(request))
        for index, original in enumerate(prepared.get("operations", [])):
            # Python callers may repeat the same dict object. A single deepcopy
            # preserves that alias, so isolate each occurrence before assigning
            # its unique owned artifact path.
            operation = copy.deepcopy(original)
            prepared["operations"][index] = operation
            if operation.get("op") == "three_dm_curve_interchange":
                operation["artifact_path"] = str(Path(job) / f"curve-{index}.3dm")
            elif operation.get("op") == "three_dm_brep_interchange":
                operation["artifact_path"] = str(Path(job) / f"brep-{index}.3dm")
            elif operation.get("op") == "border_command":
                source = operation.get("source")
                if not isinstance(source, Mapping):
                    raise OracleProtocolError("border artifact setup requires a source object")
                if source.get("type") in ("box", "sphere", "extrusion", "brep", "mesh_brep", "surface_face", "tube"):
                    operation["artifact_path"] = str(Path(job) / f"border-{index}.3dm")
            elif operation.get("op") == "cap_command":
                operation["artifact_path"] = str(Path(job) / f"cap-{index}.3dm")
            elif operation.get("op") == "brep_solid_orientation":
                operation["artifact_path"] = str(Path(job) / f"orientation-{index}.3dm")
            elif operation.get("op") in ("document_brep", "document_brep_import"):
                operation["artifact_path"] = str(Path(job) / f"document-brep-{index}.3dm")
            elif operation.get("op") in ("brep_merge_edge", "brep_remove_holes", "brep_unjoin_edges"):
                source = operation.get("source")
                if not isinstance(source, Mapping):
                    raise OracleProtocolError("B-rep operation requires a source object")
                source["artifact_path"] = str(Path(job) / f"brep-operation-{index}.3dm")
            elif operation.get("op") == "brep_join":
                operation["artifact_paths"] = [str(Path(job) / f"join-{index}-{part}.3dm")
                    for part in range(len(_artifact_sources(operation)))]
            elif operation.get("op") in ("join_command", "merge_edges_command", "merge_edge_command", "split_edge_command", "untrim_holes_command", "unjoin_edge_command", "untrim_command", "shrink_trimmed_srf_command", "shrink_trimmed_srf_to_edge_command", "extract_srf_command", "copy_options_command"):
                for part, original_source in enumerate(_artifact_sources(operation)):
                    source = copy.deepcopy(original_source)
                    operation["sources"][part] = source
                    if "brep" in source:
                        if not isinstance(source["brep"], Mapping):
                            raise OracleProtocolError("join artifact setup requires a B-rep source object")
                        source["brep"]["artifact_path"] = str(Path(job) / f"join-command-{index}-{part}.3dm")
        yield prepared


def _artifact_sources(operation):
    sources = operation.get("sources")
    if not isinstance(sources, list) or any(not isinstance(s, Mapping) for s in sources):
        raise OracleProtocolError("join artifact setup requires an array of source objects")
    return sources


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
        local_value=result['value']
        native_value=rhino_result['value']
        if isinstance(local_value,dict) and local_value.get('comparison_policy')=='block_context_member_permutation':
            if not isinstance(native_value,dict) or native_value.get('comparison_policy')!=local_value['comparison_policy']:
                raise OracleProtocolError('native context comparison policy mismatch')
            from .block_context_comparison import canonical
            local_value=canonical(local_value);native_value=canonical(native_value)
        max_error = _compare_value(
            f"{operation_id}.value",
            local_value,
            native_value,
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


def _owned_worker_exited(pids: set[int], progress_path: Path) -> bool:
    """A started worker whose observed processes are all gone is terminal.

    No PID seen yet, missing procfs, or merely a quiet progress log is not proof
    of exit. The caller rechecks the atomic response after leaving its loop.
    """
    return bool(pids) and progress_path.is_file() and _wait_for_process_exit(pids, 0.0)


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


def _validate_response(
    response: Mapping[str, Any], engine: str, request: Mapping[str, Any] | None = None,
) -> None:
    if not isinstance(response, Mapping):
        raise OracleProtocolError(f"{engine} response must be a JSON object")
    if type(response.get("protocol_version")) is not int or response["protocol_version"] != PROTOCOL_VERSION:
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
    if request is not None:
        expected = {op["id"] for op in request.get("operations", [])}
        if seen != expected:
            raise OracleProtocolError(
                f"{engine} result ids differ from request: "
                f"missing={sorted(expected - seen)!r}, unexpected={sorted(seen - expected)!r}"
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
