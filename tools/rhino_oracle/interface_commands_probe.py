# -*- coding: utf-8 -*-
"""Bounded interface commands and restoration of native settings and views."""
import math
try:
    string_types=(basestring,)
except NameError:
    string_types=(str,)

def script(command):
    """Whitelist interface-only input; never forward an unrestricted macro."""
    if not isinstance(command, string_types) or not 1 <= len(command) <= 512:
        raise ValueError("invalid interface command")
    tokens = command.split()
    if not tokens:
        raise ValueError("empty interface command")
    name = tokens.pop(0).lstrip("'_- ").lower()
    if name == "subddisplaytoggle" and not tokens:
        return "_SubDDisplayToggle"
    switches = {"setsnap": "SetSnap", "smarttrack": "SmartTrack",
                "setortho": "SetOrtho", "setplanar": "SetPlanar"}
    if name in ("snap", "ortho", "planar") and not tokens:
        return "_" + name.title()
    if name in switches and len(tokens) == 1 and tokens[0].lstrip("_").lower() in ("on", "off", "toggle"):
        return "_%s _%s" % (switches[name], tokens[0].lstrip("_").title())
    if name == "orthoangle" and len(tokens) == 1:
        try:
            angle = float(tokens[0])
        except ValueError:
            raise ValueError("invalid ortho angle")
        if math.isnan(angle) or math.isinf(angle) or not 0 < angle <= 180:
            raise ValueError("invalid ortho angle")
        return "_OrthoAngle %.17g" % angle
    if name == "orthosnaptocplanez" and len(tokens) == 1 and tokens[0].lstrip("_").lower() in ("enable", "disable", "toggle"):
        return "_OrthoSnapToCPlaneZ _%s" % tokens[0].lstrip("_").title()
    if name == "disableosnap" and len(tokens) == 1 and tokens[0].lstrip("_").lower() in ("enable", "disable", "toggle"):
        return "_DisableOsnap _%s" % tokens[0].lstrip("_").title()
    if name == "snaptomeshes" and len(tokens) == 1 and tokens[0].lstrip("_").lower() in ("enable", "disable", "toggle"):
        return "_SnapToMeshes _%s" % tokens[0].lstrip("_").title()
    if name != "setdisplaymode":
        raise ValueError("unsupported interface command or options")
    options = {}
    while tokens:
        token = tokens.pop(0)
        if "=" in token:
            key, value = token.split("=", 1)
        elif token.lstrip("_").lower() in ("viewport", "mode"):
            if not tokens:
                raise ValueError("missing interface option value")
            key, value = token, tokens.pop(0)
        else:
            key, value = "mode", token
        key, value = key.lstrip("_").lower(), value.lstrip("_").lower()
        if key in options or key not in ("viewport", "mode"):
            raise ValueError("duplicate or unknown interface option")
        options[key] = value
    mode, viewport = options.get("mode"), options.get("viewport", "active")
    if mode not in ("wireframe", "shaded", "ghosted") or viewport not in ("active", "all"):
        raise ValueError("unsupported interface mode or viewport")
    # Mode finishes the command, so target the viewport before supplying it.
    return "_-SetDisplayMode _Viewport=_%s _Mode=_%s" % (viewport.title(), mode.title())


def state(views, aid, track, extras, host):
    Rhino=host["Rhino"]
    active = Rhino.RhinoDoc.ActiveDoc.Views.ActiveView
    value = {"grid_snap": bool(aid.GridSnap), "osnap": bool(aid.Osnap),
            "smart_track": bool(track.UseSmartTrack),
            "active_viewport": next(i for i, view in enumerate(views) if view.ActiveViewportID == active.ActiveViewportID),
            "display_modes": [view.ActiveViewport.DisplayMode.EnglishName for view in views]}
    if "ortho" in extras:
        value["ortho"] = bool(aid.Ortho)
    if "planar" in extras:
        value["planar"] = bool(aid.Planar)
    if "ortho_angle_degrees" in extras:
        value["ortho_angle_degrees"] = math.degrees(float(aid.OrthoAngle))
    if "ortho_snap_to_cplane_z" in extras:
        value["ortho_snap_to_cplane_z"] = bool(aid.OrthoUseZ)
    return value


def run(operation, host):
    _interface_script=host["_interface_script"]
    commands = operation["commands"]
    if not isinstance(commands, list) or not 1 <= len(commands) <= 128:
        raise ValueError("expected 1 to 128 interface commands")
    scripts = [_interface_script(command) for command in commands]
    mesh_requested = "snap_to_meshes" in operation
    if mesh_requested and type(operation["snap_to_meshes"]) is not bool:
        raise ValueError("snap_to_meshes must be boolean")
    if not mesh_requested and any(script.startswith("_SnapToMeshes ") for script in scripts):
        raise ValueError("mesh snap commands require an explicit initial setting")
    subd_requested = "subd_control_net" in operation
    if subd_requested and type(operation["subd_control_net"]) is not bool:
        raise ValueError("subd_control_net must be boolean")
    if not subd_requested and "_SubDDisplayToggle" in scripts:
        raise ValueError("SubD display commands require an explicit initial setting")
    aid_keys = ("ortho", "planar", "ortho_angle_degrees", "ortho_snap_to_cplane_z")
    aid_commands = {"ortho": ("_Ortho", "_SetOrtho "),
                    "planar": ("_Planar", "_SetPlanar "),
                    "ortho_angle_degrees": ("_OrthoAngle ",),
                    "ortho_snap_to_cplane_z": ("_OrthoSnapToCPlaneZ ",)}
    for key in aid_keys:
        if key in operation:
            value = operation[key]
            if key == "ortho_angle_degrees":
                if isinstance(value, bool) or not isinstance(value, (int, float)) or math.isnan(value) or math.isinf(value) or not 0 < value <= 180:
                    raise ValueError("invalid initial ortho angle")
            elif type(value) is not bool:
                raise ValueError("interface aid flags must be booleans")
        elif any(script == prefix or (prefix.endswith(" ") and script.startswith(prefix))
                 for script in scripts for prefix in aid_commands[key]):
            raise ValueError("interface aid commands require an explicit initial setting")
    if any(not isinstance(operation[key], bool) for key in ("grid_snap", "osnap", "smart_track")):
        raise ValueError("interface flags must be booleans")
    index = operation["active_viewport"]
    if isinstance(index, bool) or not isinstance(index, int) or not 0 <= index < 4:
        raise ValueError("invalid active viewport index")
    names = operation["display_modes"]
    if not isinstance(names, list) or len(names) != 4 or any(name not in ("Wireframe", "Shaded", "Ghosted") for name in names):
        raise ValueError("expected four supported display modes")
    Rhino=host["Rhino"]
    _interface_state=host["_interface_state"]
    _record_progress=host["_record_progress"]
    _run_surface_script=host["_run_surface_script"]
    modes = [Rhino.Display.DisplayModeDescription.FindByName(name) for name in names]
    if any(mode is None for mode in modes):
        raise ValueError("display mode unavailable")
    document = Rhino.RhinoDoc.ActiveDoc
    views = list(document.Views.GetViewList(True, False))
    if len(views) != 4:
        raise ValueError("interface probes require four model views")
    aid = Rhino.ApplicationSettings.ModelAidSettings
    track = Rhino.ApplicationSettings.SmartTrackSettings
    original_aid = aid.GetCurrentState()
    original_track = track.GetCurrentState()
    original_modes = [view.ActiveViewport.DisplayMode for view in views]
    original_view = document.Views.ActiveView
    original_subd = document.SubDAppearance if subd_requested else None
    original_mesh = None
    def record():
        value = _interface_state(views, aid, track, tuple(key for key in aid_keys if key in operation))
        if mesh_requested:
            value["snap_to_meshes"] = mesh_snap_settings_probe.current(host)
        if subd_requested:
            value["subd_control_net"] = document.SubDAppearance == Rhino.Geometry.SubDComponentLocation.ControlNet
        return value
    try:
        if subd_requested:
            document.SubDAppearance = (Rhino.Geometry.SubDComponentLocation.ControlNet if operation["subd_control_net"]
                else Rhino.Geometry.SubDComponentLocation.Surface)
        if mesh_requested:
            import mesh_snap_settings_probe
            original_mesh = mesh_snap_settings_probe.current(host)
            mesh_snap_settings_probe.set_enabled(operation["snap_to_meshes"], host)
        aid.GridSnap = operation["grid_snap"]
        aid.Osnap = operation["osnap"]
        if "ortho" in operation:
            aid.Ortho = operation["ortho"]
        if "planar" in operation:
            aid.Planar = operation["planar"]
        if "ortho_angle_degrees" in operation:
            aid.OrthoAngle = math.radians(operation["ortho_angle_degrees"])
        if "ortho_snap_to_cplane_z" in operation:
            aid.OrthoUseZ = operation["ortho_snap_to_cplane_z"]
        track.UseSmartTrack = operation["smart_track"]
        for view, mode in zip(views, modes):
            view.ActiveViewport.DisplayMode = mode
        document.Views.ActiveView = views[index]
        states = [record()]
        for script in scripts:
            _record_progress("interface command: " + script)
            if not _run_surface_script(script, True):
                raise ValueError("interface command failed: " + script)
            states.append(record())
        return {"states": states}, 0
    finally:
        # Every restoration is attempted, even if cancellation or another
        # setting fails. Keep the first cleanup failure for the caller.
        actions = [lambda: Rhino.RhinoApp.RunScript("!", False),
            lambda: aid.UpdateFromState(original_aid),
            lambda: track.UpdateFromState(original_track)]
        actions.extend(lambda view=view,mode=mode: setattr(view.ActiveViewport,"DisplayMode",mode)
            for view,mode in zip(views,original_modes))
        actions.append(lambda: setattr(document.Views,"ActiveView",original_view))
        if original_mesh is not None:
            actions.append(lambda: mesh_snap_settings_probe.set_enabled(original_mesh,host))
        if original_subd is not None:
            actions.append(lambda: setattr(document,"SubDAppearance",original_subd))
        errors=[]
        for restore in actions:
            try:restore()
            except Exception as error:errors.append(error)
        if errors:raise errors[0]

