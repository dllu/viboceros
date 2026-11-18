from __future__ import annotations

from dataclasses import asdict, dataclass
from math import isfinite
from numbers import Real
from typing import Any, Literal
from uuid import UUID

XYZ = tuple[float, float, float]
View = int | str
Button = Literal["primary", "secondary", "middle"]
PointerAction = Literal["click", "move", "press", "release"]


class SessionError(RuntimeError):
    """The headless session could not complete an API request."""


class CommandError(SessionError):
    """A command or input failed. The completed steps are available in result."""

    def __init__(self, result: CommandResult) -> None:
        self.result = result
        super().__init__("; ".join(result.errors))


@dataclass(frozen=True)
class Object:
    id: str
    kind: str
    name: str | None
    layer: str
    groups: tuple[str, ...]
    visible: bool
    locked: bool
    selected: bool
    bounds: tuple[XYZ, XYZ] | None

    @classmethod
    def _read(cls, record: dict[str, Any]) -> Object:
        bounds = record["bounds"]
        return cls(**{
            **record,
            "bounds": tuple(map(tuple, bounds)) if bounds is not None else None,
            "groups": tuple(record["groups"]),
        })


ObjectLike = Object | str | UUID
CommandArgument = ObjectLike | Real | XYZ


def object_id(obj: ObjectLike) -> str:
    return obj.id if isinstance(obj, Object) else str(obj)


def command_token(value: CommandArgument) -> str:
    """Format one regular command argument using native command syntax."""
    if isinstance(value, bool):
        return "Yes" if value else "No"
    if isinstance(value, Real):
        if not isfinite(value):
            raise ValueError("command numbers must be finite")
        return str(value)
    if isinstance(value, (tuple, list)) and len(value) == 3:
        if any(isinstance(coordinate, bool) or not isinstance(coordinate, Real) for coordinate in value):
            raise TypeError("point arguments must have three numeric coordinates")
        return ",".join(map(command_token, value))
    if isinstance(value, (str, UUID, Object)):
        token = object_id(value)
        if any(character in token for character in '"\r\n\0'):
            raise ValueError("pass arguments with special command syntax directly to run()")
        return f'"{token}"' if not token or any(character.isspace() for character in token) else token
    raise TypeError(f"unsupported command argument {value!r}")


@dataclass(frozen=True)
class Component:
    object: ObjectLike
    kind: str
    index: int
    points: tuple[XYZ, ...] = ()
    loops: tuple[tuple[int, ...], ...] = ()
    revision: int | None = None

    @classmethod
    def _read(cls, record: dict[str, Any]) -> Component:
        return cls(
            record["object"], record["kind"], record["index"],
            tuple(map(tuple, record.get("points", ()))),
            tuple(map(tuple, record.get("loops", ()))),
            record.get("revision"),
        )

    def _ref(self) -> dict[str, Any]:
        return {"object": object_id(self.object), "kind": self.kind, "index": self.index, "revision": self.revision}


@dataclass(frozen=True)
class Grip:
    object: ObjectLike
    index: int
    point: XYZ | None = None
    selected: bool = False
    hidden: bool = False
    revision: int | None = None
    mode: str | None = None

    @classmethod
    def _read(cls, record: dict[str, Any]) -> Grip:
        if "point" in record:
            record = {**record, "point": tuple(record["point"])}
        return cls(**record)

    def _ref(self) -> dict[str, Any]:
        return {"object": object_id(self.object), "index": self.index, "revision": self.revision, "mode": self.mode}


@dataclass(frozen=True)
class Selection:
    objects: tuple[str, ...]
    components: tuple[Component, ...]
    grips: tuple[Grip, ...]

    @classmethod
    def _read(cls, record: dict[str, Any]) -> Selection:
        return cls(
            tuple(record["objects"]),
            tuple(map(Component._read, record["components"])),
            tuple(map(Grip._read, record["grips"])),
        )


@dataclass(frozen=True)
class Camera:
    """A perspective capture camera in world coordinates."""

    eye: XYZ
    target: XYZ
    up: XYZ = (0.0, 0.0, 1.0)
    vertical_fov_degrees: float = 34.0

    def _payload(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class Viewport:
    index: int
    name: str
    kind: str
    mode: str
    active: bool
    rectangle: tuple[float, float, float, float] | None

    @classmethod
    def _read(cls, record: dict[str, Any]) -> Viewport:
        rect = record["rectangle"]
        return cls(**{**record, "rectangle": tuple(rect) if rect is not None else None})


@dataclass(frozen=True)
class Widget:
    id: int
    label: str | None
    role: str
    value: str | None
    enabled: bool
    clickable: bool
    focusable: bool
    rectangle: tuple[float, float, float, float] | None

    @classmethod
    def _read(cls, record: dict[str, Any]) -> Widget:
        rect = record["rectangle"]
        return cls(**{**record, "rectangle": tuple(rect) if rect is not None else None})


@dataclass(frozen=True)
class CommandResult:
    messages: tuple[str, ...]
    errors: tuple[str, ...]
    processed: int
    pending: bool
    prompt: str | None
    selected: tuple[str, ...]
    added: tuple[str, ...]
    modified: tuple[str, ...]
    deleted: tuple[str, ...]

    @classmethod
    def _read(cls, record: dict[str, Any]) -> CommandResult:
        changes = record["changes"] or {}
        return cls(
            tuple(record["messages"]), tuple(record["errors"]), record["processed"],
            record["pending"], record["prompt"], tuple(record["selected"]), tuple(changes.get("added", ())),
            tuple(changes.get("modified", ())), tuple(changes.get("deleted", ())),
        )


@dataclass(frozen=True)
class Point:
    """A typed point, in world coordinates by default."""

    x: float
    y: float
    z: float = 0.0
    world: bool = True


@dataclass(frozen=True)
class Pick:
    """Pick an object, component, or grip by identity in the current getter."""

    target: ObjectLike | Component | Grip
    remove: bool = False
    at: XYZ | None = None


@dataclass(frozen=True)
class PickPoint:
    """Project a world point into a viewport and click there."""

    point: XYZ
    view: View | None = None
    shift: bool = False
    ctrl: bool = False
    alt: bool = False
    command: bool = False


@dataclass(frozen=True)
class Pointer:
    """Use window coordinates, including viewport positions and dialog controls."""

    x: float
    y: float
    action: PointerAction = "click"
    button: Button = "primary"
    shift: bool = False
    ctrl: bool = False
    alt: bool = False
    command: bool = False


@dataclass(frozen=True)
class Key:
    key: str
    shift: bool = False
    ctrl: bool = False
    alt: bool = False
    command: bool = False


@dataclass(frozen=True)
class ActivateView:
    view: View


@dataclass(frozen=True)
class TypeText:
    """Type into the currently focused widget without submitting a command."""

    text: str


@dataclass(frozen=True)
class WidgetAction:
    """Click or focus a UI control by its label or widget ID."""

    target: Widget | int | str
    action: Literal["click", "focus"] = "click"


ENTER = None
ESCAPE = Key("Escape")

Step = str | float | XYZ | Point | Pick | PickPoint | Pointer | Key | ActivateView | TypeText | WidgetAction | None


def encode_step(step: Step) -> dict[str, Any]:
    if step is None or isinstance(step, (str, Real)):
        return {"kind": "text", "text": "" if step is None else str(step)}
    if isinstance(step, (tuple, list)):
        step = Point(*step)
    if isinstance(step, Point):
        return {"kind": "point", "point": (step.x, step.y, step.z), "world": step.world}
    if isinstance(step, Pick):
        if isinstance(step.target, Component):
            return {"kind": "pick_component", "component": step.target._ref(), "point": step.at, "remove": step.remove}
        if step.at is not None:
            raise ValueError("at supplies the world location for a component pick")
        if isinstance(step.target, Grip):
            return {"kind": "pick_grip", "grip": step.target._ref(), "remove": step.remove}
        return {"kind": "pick", "id": object_id(step.target), "remove": step.remove}
    if isinstance(step, ActivateView):
        return {"kind": "activate_view", "view": step.view}
    if isinstance(step, TypeText):
        return {"kind": "type_text", "text": step.text}
    if isinstance(step, WidgetAction):
        target = step.target.id if isinstance(step.target, Widget) else step.target
        return {"kind": "widget_action", "target": target, "action": step.action}
    if isinstance(step, (PickPoint, Pointer, Key)):
        result: dict[str, Any] = {
            "modifiers": {name: getattr(step, name) for name in ("shift", "ctrl", "alt", "command")},
        }
        if isinstance(step, PickPoint):
            result.update(kind="pick_point", point=step.point, view=step.view)
        elif isinstance(step, Pointer):
            result.update(kind="pointer", position=(step.x, step.y), action=step.action, button=step.button)
        else:
            result.update(kind="key", key=step.key)
        return result
    raise TypeError(f"unsupported modeling input {step!r}")
