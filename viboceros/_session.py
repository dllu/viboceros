from __future__ import annotations

import json
import os
import shutil
import subprocess
from collections.abc import Mapping, Sequence
from pathlib import Path
from threading import Event, Lock, Timer
from typing import Any, Literal

from ._surfaces import Domain, NurbsSurface, RGB, SurfaceJet, SurfacePatch, SurfaceSample, TrimCurve, UV
from ._types import (
    Camera, CommandArgument, CommandError, CommandResult, Component, Grip, Object, ObjectLike, Selection,
    SessionError, Step, View, Viewport, Widget, XYZ, command_token, encode_step, object_id,
)

PathLike = str | os.PathLike[str]
_PROTOCOL = 1
_FORMATS = {".3dm": "3dm", ".stl": "Stl", ".step": "Step", ".stp": "Step"}


def _executable(explicit: PathLike | None) -> str:
    configured = explicit if explicit is not None else os.environ.get("VIBOCEROS_BIN")
    if configured is not None:
        return os.fspath(Path(configured).expanduser().absolute())
    if path := shutil.which("viboceros"):
        return path
    name = "viboceros.exe" if os.name == "nt" else "viboceros"
    for root in (Path(__file__).resolve().parent.parent, Path.cwd()):
        paths = [root / "target" / profile / name for profile in ("release", "debug")]
        built = [path for path in paths if path.is_file()]
        if built:
            return str(max(built, key=lambda path: path.stat().st_mtime_ns))
    raise SessionError("Viboceros executable not found. Build it with cargo build --release or set VIBOCEROS_BIN.")


def _path(path: PathLike) -> Path:
    return Path(path).expanduser().absolute()


class Session:
    """A persistent Viboceros model, command interpreter, and viewport set.

    Each session owns a native process. Use a context manager to close it when
    the script finishes. All path arguments are resolved by the Python caller.
    """

    def __init__(self, executable: PathLike | None = None) -> None:
        self._lock = Lock()
        self._closed = False
        environment = os.environ.copy()
        environment.pop("DISPLAY", None)
        environment.pop("WAYLAND_DISPLAY", None)
        try:
            self._process = subprocess.Popen(
                [_executable(executable), "--headless"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                encoding="utf-8", errors="strict", env=environment,
            )
        except OSError as error:
            raise SessionError(f"could not start Viboceros: {error}") from error
        expired = Event()

        def stop_startup() -> None:
            expired.set()
            self._process.kill()

        timer = Timer(15, stop_startup)
        timer.daemon = True
        timer.start()
        try:
            info = self.info()
            if info.get("protocol") != _PROTOCOL:
                raise SessionError(f"incompatible Viboceros protocol {info.get('protocol')!r}")
        except BaseException:
            self.close()
            if expired.is_set():
                raise SessionError("Viboceros did not start its headless protocol within 15 seconds") from None
            raise
        finally:
            timer.cancel()

    def __enter__(self) -> Session:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def close(self) -> None:
        """Release the document and native process."""
        with self._lock:
            if self._closed:
                return
            self._closed = True
            process = self._process
            try:
                process.stdin.close()
            except BrokenPipeError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            finally:
                process.stdout.close()

    def _request(self, method: str, **params: Any) -> Any:
        payload = json.dumps({"method": method, **params}, allow_nan=False)
        with self._lock:
            if self._closed:
                raise SessionError("Viboceros session is closed")
            process = self._process
            try:
                process.stdin.write(payload + "\n")
                process.stdin.flush()
                line = process.stdout.readline()
            except OSError as error:
                raise SessionError(f"Viboceros communication failed: {error}") from error
            except KeyboardInterrupt:
                # An abandoned response would otherwise be read by the next request.
                process.kill()
                raise
            if not line:
                raise SessionError("Viboceros headless process closed without a response")
            try:
                response = json.loads(line)
            except ValueError as error:
                raise SessionError("Viboceros sent an invalid headless response") from error
            if "error" in response:
                raise SessionError(response["error"])
            return response["result"]

    def info(self) -> dict[str, Any]:
        """Return version, backend, project path, units, and getter state."""
        return self._request("info")

    def new(self) -> None:
        """Start a new document and discard the session's current unsaved state."""
        self._request("reset")

    def commands(self) -> tuple[str, ...]:
        """Return command names from Viboceros's command completion catalog."""
        return tuple(self._request("commands"))

    def run(
        self, *steps: Step, select: Sequence[ObjectLike] | None = None,
        check: bool = True, track_changes: bool = True,
    ) -> CommandResult:
        """Submit whole command lines, prompt answers, points, or picks in order.

        Strings are submitted as one line. Tuple points use world coordinates.
        ENTER finishes variable-length picks. A pending result can be continued
        by calling run again. With check enabled, processing stops at the first
        reported error and raises CommandError with the partial result.
        """
        result = CommandResult._read(self._request(
            "run", steps=[encode_step(step) for step in steps],
            select=None if select is None else list(map(object_id, select)),
            track_changes=track_changes, stop_on_error=check,
        ))
        if check and result.errors:
            raise CommandError(result)
        return result

    def command(
        self, name: str, *arguments: CommandArgument,
        options: Mapping[str, CommandArgument] | None = None,
        select: Sequence[ObjectLike] | None = None,
    ) -> CommandResult:
        """Run a command with numbers, points, object IDs, and named options."""
        if not name or any(character.isspace() or character in '"=\0' for character in name):
            raise ValueError("pass one command name, with arguments supplied separately")
        tokens = [name, *map(command_token, arguments)]
        for key, value in (options or {}).items():
            if not key or any(character.isspace() or character in '"=\0' for character in key):
                raise ValueError(f"invalid command option name {key!r}")
            tokens.append(f"{key}={command_token(value)}")
        return self.run(" ".join(tokens), select=select)

    def objects(
        self, *objects: ObjectLike, kind: str | None = None, name: str | None = None,
        layer: str | None = None,
    ) -> tuple[Object, ...]:
        """Read object identities, attributes, and display bounds in document order."""
        if layer is not None:
            layer = next((item["id"] for item in self.layers() if item["name"] == layer), layer)
        records = self._request("objects", ids=list(map(object_id, objects)) if objects else None)
        return tuple(
            obj for obj in map(Object._read, records)
            if (kind is None or obj.kind == kind)
            and (name is None or obj.name == name)
            and (layer is None or obj.layer == layer)
        )

    def object(self, obj: ObjectLike) -> Object:
        return self.objects(obj)[0]

    def layers(self) -> tuple[dict[str, Any], ...]:
        return tuple(self._request("layers"))

    def groups(self) -> tuple[dict[str, Any], ...]:
        return tuple(self._request("groups"))

    def selection(self) -> Selection:
        return Selection._read(self._request("selection"))

    def select(self, *objects: ObjectLike) -> Selection:
        """Set the whole-object preselection in argument order. No arguments clears it."""
        return Selection._read(self._request("select", ids=list(map(object_id, objects))))

    def components(self, obj: ObjectLike) -> tuple[Component, ...]:
        """List selectable topology (zero-based B-rep, mesh, or SubD indices)."""
        return tuple(map(Component._read, self._request("components", id=object_id(obj))))

    def select_components(self, *components: Component) -> Selection:
        """Preselect specific components for a modeling command."""
        return Selection._read(self._request("select_components", components=[c._ref() for c in components]))

    def grips(self, obj: ObjectLike) -> tuple[Grip, ...]:
        """List active grips after PointsOn or EditPtOn."""
        return tuple(map(Grip._read, self._request("grips", id=object_id(obj))))

    def select_grips(self, *grips: Grip) -> Selection:
        return Selection._read(self._request("select_grips", grips=[g._ref() for g in grips]))

    def open(self, path: PathLike) -> CommandResult:
        """Open a 3DM project, replacing the current document."""
        return self._file_command(f"Open3dm {_path(path)}", track_changes=True)

    def save(self, path: PathLike | None = None) -> CommandResult:
        """Save the current 3DM project, including its views and construction planes."""
        return self._file_command("Save" if path is None else f"SaveAs {_path(path)}", track_changes=False)

    def import_file(self, path: PathLike, *, native_step: bool = False) -> CommandResult:
        """Import 3DM, STL, or STEP into the current document."""
        path = _path(path)
        form = _FORMATS.get(path.suffix.lower())
        if form is None or native_step and form != "Step":
            raise ValueError("expected 3DM, STL, or STEP (native_step applies only to STEP)")
        options = "Native=Yes " if native_step else ""
        return self._file_command(f"Import{form} {options}{path}", track_changes=True)

    def export(
        self, path: PathLike, *, native_step: bool = False, ascii_stl: bool = False,
        selected_only: bool = False,
    ) -> CommandResult:
        """Export to 3DM, STL, or STEP. Use selected_only to export the current 3DM selection."""
        path = _path(path)
        form = _FORMATS.get(path.suffix.lower())
        if (form is None or native_step and form != "Step" or ascii_stl and form != "Stl"
                or selected_only and form != "3dm"):
            raise ValueError("expected 3DM, STL, or STEP with options matching the file type")
        options = "Native=Yes " if native_step else "Ascii " if ascii_stl else ""
        if selected_only:
            options = "Selected=Yes "
        return self._file_command(f"Export{form} {options}{path}", track_changes=False)

    def _file_command(self, command: str, *, track_changes: bool) -> CommandResult:
        result = CommandResult._read(self._request(
            "run", steps=[encode_step(command)], select=None, track_changes=track_changes,
            stop_on_error=True, require_idle=True,
        ))
        if result.errors:
            raise CommandError(result)
        return result

    def add_surface_patches(self, surface: NurbsSurface, *patches: SurfacePatch) -> tuple[str, ...]:
        """Add exact restrictions of one surface in a single undoable edit."""
        return tuple(self._request(
            "add_surface_patches", surface=surface._payload(),
            patches=[patch._payload() for patch in patches],
        ))

    def add_nurbs_surface(
        self, surface: NurbsSurface, *, domain: Domain | None = None,
        name: str | None = None, color: RGB | None = None,
        trim_loops: Sequence[Sequence[TrimCurve]] = (),
        opacity: float | None = None,
    ) -> str:
        """Add a NURBS surface using explicit knots and control points."""
        return self.add_surface_patches(surface, SurfacePatch(domain, name, color, trim_loops, opacity))[0]

    def surface_definition(self, obj: ObjectLike, *, face: int | None = None) -> NurbsSurface:
        """Read the tensor-product definition of a surface or B-rep face."""
        return NurbsSurface(**self._request("surface_definition", id=object_id(obj), face=face))

    def sew_brep_edges(
        self, *pairs: tuple[Component, Component, bool], anchor: ObjectLike | None = None,
    ) -> str:
        """Sew explicit edge pairs and replace their B-reps in one undoable edit.

        The boolean is true when the second edge runs opposite the first.
        The result inherits the attributes and orientation of the anchor
        object, or of the first referenced object when omitted.
        """
        return self._request("sew_brep_edges", pairs=[
            (a._ref(), b._ref(), reversed) for a, b, reversed in pairs
        ], anchor=None if anchor is None else object_id(anchor))

    def surface_jets(
        self, obj: ObjectLike, *samples: SurfaceSample | UV, face: int | None = None,
    ) -> tuple[SurfaceJet, ...]:
        """Evaluate position and first and second partials at native UV coordinates."""
        records = self._request(
            "surface_jets", id=object_id(obj), face=face,
            samples=[(sample if isinstance(sample, SurfaceSample) else SurfaceSample(sample))._payload()
                     for sample in samples],
        )
        return tuple(SurfaceJet(**{name: tuple(value) for name, value in jet.items()}) for jet in records)

    def viewports(self) -> tuple[Viewport, ...]:
        return tuple(map(Viewport._read, self._request("viewports")))

    def widgets(self) -> tuple[Widget, ...]:
        """List the current command and dialog controls, including their labels."""
        return tuple(map(Widget._read, self._request("widgets")))

    def project(self, point: XYZ, *, view: View | None = None) -> tuple[float, float]:
        """Project a visible world point into headless window coordinates."""
        x, y = self._request("project", point=point, view=view)
        return x, y

    def screenshot(
        self, path: PathLike, *, view: View | None = None, mode: str = "Shaded",
        size: tuple[int, int] = (1200, 800), fit: bool = True,
        background: tuple[int, int, int, int] = (255, 255, 255, 255),
        camera: Camera | None = None, show_edges: bool = True,
        lighting: Literal["viewport", "diffuse"] = "viewport",
    ) -> Path:
        """Write a PNG using a viewport, named view, or standard view direction."""
        path = _path(path)
        self._request(
            "screenshot", path=str(path), view=view, mode=mode, size=size,
            fit=fit, background=background, camera=None if camera is None else camera._payload(),
            show_edges=show_edges, lighting=lighting,
        )
        return path
