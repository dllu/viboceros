"""Bounded public-command diagnostic for Rhino's mesh-snap switch grammar."""
from contextlib import contextmanager
import re


def current(host):
    app = host["Rhino"].RhinoApp
    before = app.CommandHistoryWindowText
    app.RunScript("_SnapToMeshes _Cancel", True)
    history = app.CommandHistoryWindowText[len(before):]
    values = re.findall(r"Mesh object Near, Mid, Int, and Perp osnap support is (enabled|disabled)\.", history)
    if not values:
        raise ValueError("cannot read mesh snap state from the English public command prompt: " + history)
    return values[-1] == "enabled"


def set_enabled(enabled, host):
    host["Rhino"].RhinoApp.RunScript("_SnapToMeshes _" + ("Enable" if enabled else "Disable"), True)
    if current(host) != enabled:
        raise ValueError("Rhino did not accept the requested mesh snap setting")


@contextmanager
def environment(enabled, host):
    if type(enabled) is not bool:
        raise ValueError("mesh snap setting must be boolean")
    original = current(host)
    state = dict(before=original, requested=enabled)
    try:
        set_enabled(enabled, host)
        yield state
    finally:
        set_enabled(original, host)
        state["restored"] = original


def run(operation, host):
    if set(operation) != set(("op", "id")):
        raise ValueError("mesh snap settings diagnostic accepts no arguments")
    Rhino = host["Rhino"]
    histories = []
    # Cancel any option prompt. If a Rhino version instead toggles immediately,
    # the second invocation restores that state; no arbitrary script is accepted.
    for _ in range(2):
        before = Rhino.RhinoApp.CommandHistoryWindowText
        result = Rhino.RhinoApp.RunScript("_SnapToMeshes _Cancel", True)
        after = Rhino.RhinoApp.CommandHistoryWindowText
        histories.append(dict(result=bool(result), history=after[len(before):]))
    return dict(commands=histories), 0
