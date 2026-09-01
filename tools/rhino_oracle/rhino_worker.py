"""Standalone RhinoPython worker for the versioned compatibility oracle.

This file is copied beside request.json and executed inside Rhino. Keep its
syntax compatible with both Rhino 8 Python 3 and the legacy IronPython host.
"""

import json
import math
import os
from timeit import default_timer

import Rhino
import System


PROTOCOL_VERSION = 1
MAX_ITERATIONS = 1000000
MAX_STATE_CYCLE_OBJECTS = 100000
DEFAULT_TOLERANCE = {
    "absolute": 1.0e-9,
    "relative": 1.0e-12,
    "angular": 1.0e-10,
}
try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)
try:
    iteration_range = xrange
except NameError:
    iteration_range = range


def _point(coordinates):
    values = [_finite(value, "point coordinate") for value in coordinates]
    if len(values) != 3:
        raise ValueError("point must contain exactly three coordinates")
    return Rhino.Geometry.Point3d(values[0], values[1], values[2])


def _vector(coordinates):
    values = [_finite(value, "vector coordinate") for value in coordinates]
    if len(values) != 3:
        raise ValueError("vector must contain exactly three coordinates")
    return Rhino.Geometry.Vector3d(values[0], values[1], values[2])


def _xyz(value):
    return [float(value.X), float(value.Y), float(value.Z)]


def _finite(value, context):
    number = float(value)
    if math.isnan(number) or math.isinf(number):
        raise ValueError("%s must be finite" % context)
    return number


def _wildcard_matches(pattern, candidate):
    pattern = pattern.lower()
    candidate = candidate.lower()
    pattern_index = 0
    candidate_index = 0
    star_index = None
    star_candidate_index = 0
    while candidate_index < len(candidate):
        if pattern_index < len(pattern) and (
            pattern[pattern_index] == "?"
            or pattern[pattern_index] == candidate[candidate_index]
        ):
            pattern_index += 1
            candidate_index += 1
        elif pattern_index < len(pattern) and pattern[pattern_index] == "*":
            star_index = pattern_index
            pattern_index += 1
            star_candidate_index = candidate_index
        elif star_index is not None:
            pattern_index = star_index + 1
            star_candidate_index += 1
            candidate_index = star_candidate_index
        else:
            return False
    while pattern_index < len(pattern) and pattern[pattern_index] == "*":
        pattern_index += 1
    return pattern_index == len(pattern)


def _state_cycle_indices(operation, name, object_count):
    values = operation.get(name)
    if not isinstance(values, list):
        raise ValueError("%s must be an array" % name)
    indices = set()
    for value in values:
        if isinstance(value, bool) or int(value) != value:
            raise ValueError("%s must contain integer indices" % name)
        index = int(value)
        if index < 0 or index >= object_count:
            raise ValueError(
                "%s index %d is outside object count %d"
                % (name, index, object_count)
            )
        indices.add(index)
    return sorted(indices)


def _unit(vector, tolerance, context):
    length = float(vector.Length)
    if not length > tolerance:
        raise ValueError("%s is degenerate" % context)
    vector /= length
    return vector


def _measure(iterations, operation):
    value = operation()
    started = default_timer()
    for _unused in iteration_range(iterations):
        value = operation()
    elapsed_ns = int(round((default_timer() - started) * 1000000000.0))
    return value, max(0, elapsed_ns)


def _canonical_join_segments(curves):
    polylines = []
    for curve in curves:
        segments = curve.DuplicateSegments()
        if segments is None or len(segments) == 0:
            values = [[_xyz(curve.PointAtStart), _xyz(curve.PointAtEnd)]]
        else:
            values = [
                [_xyz(segment.PointAtStart), _xyz(segment.PointAtEnd)]
                for segment in segments
            ]
        if tuple(values[-1][1]) < tuple(values[0][0]):
            values.reverse()
            values = [[segment[1], segment[0]] for segment in values]
        polylines.append(values)
    polylines.sort(
        key=lambda segments: tuple(
            tuple(point) for segment in segments for point in segment
        )
    )
    return polylines


def _set_knots(target, full_knots, context):
    values = [_finite(value, context) for value in full_knots]
    if len(values) != target.Count + 2:
        raise ValueError(
            "%s count must be Rhino knot count plus two" % context
        )
    for index, value in enumerate(values[1:-1]):
        target[index] = value


def _set_curve_controls(curve, controls):
    if len(controls) != curve.Points.Count:
        raise ValueError("NURBS curve control-point count does not match")
    for index, control in enumerate(controls):
        point = _point(control["point"])
        weight = _finite(control.get("weight", 1.0), "control-point weight")
        if not weight > 0.0 or not curve.Points.SetPoint(index, point, weight):
            raise ValueError("invalid NURBS curve control point")


def _set_surface_controls(surface, controls, count_u, count_v):
    if len(controls) != count_u * count_v:
        raise ValueError("NURBS surface control-net size does not match")
    for v_index in range(count_v):
        for u_index in range(count_u):
            control = controls[v_index * count_u + u_index]
            point = _point(control["point"])
            weight = _finite(control.get("weight", 1.0), "control-point weight")
            if not weight > 0.0 or not surface.Points.SetPoint(
                u_index, v_index, point, weight
            ):
                raise ValueError("invalid NURBS surface control point")


def _triangle_mesh(vertices, triangles):
    mesh = Rhino.Geometry.Mesh()
    try:
        for vertex in vertices:
            if mesh.Vertices.Add(_point(vertex)) < 0:
                raise ValueError("could not add mesh vertex")
        for triangle in triangles:
            if len(triangle) != 3:
                raise ValueError("mesh face must contain exactly three indices")
            if any(
                isinstance(index, bool) or int(index) != index for index in triangle
            ):
                raise ValueError("mesh face index must be an integer")
            indices = [int(index) for index in triangle]
            if mesh.Faces.AddFace(indices[0], indices[1], indices[2]) < 0:
                raise ValueError("could not add mesh face")
        if not mesh.IsValid:
            raise ValueError("mesh is invalid")
        return mesh
    except Exception:
        mesh.Dispose()
        raise


def _mesh_triangles(mesh):
    triangles = []
    for index in range(mesh.Faces.Count):
        face = mesh.Faces[index]
        if not face.IsTriangle:
            raise ValueError("oracle mesh unexpectedly contains a quad")
        triangles.append([int(face.A), int(face.B), int(face.C)])
    return triangles


def _mesh_value(mesh):
    return {
        "triangles": _mesh_triangles(mesh),
        "vertices": [
            _xyz(mesh.Vertices[index]) for index in range(mesh.Vertices.Count)
        ],
    }


def _execute(operation, iterations, tolerance):
    kind = operation["op"]
    if kind == "document_layer_assignment_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid()).replace("-", "")
        default_layer_index = int(document.Layers.CurrentLayerIndex)
        layer_indices = []
        object_ids = []
        original_group_index = None

        def set_layer_mode(layer_index, visible, locked):
            layer = document.Layers[layer_index]
            layer.IsVisible = visible
            layer.IsLocked = locked
            if not document.Layers.Modify(layer, layer_index, True):
                raise ValueError("could not modify layer-assignment layer")

        try:
            for label, visible, locked in (
                ("Normal", True, False),
                ("Hidden", False, False),
                ("Locked", True, True),
            ):
                layer = Rhino.DocObjects.Layer()
                layer.Name = "ViboLayerAssignment%s%s" % (label, suffix)
                layer.IsVisible = visible
                layer.IsLocked = locked
                layer_index = document.Layers.Add(layer)
                if layer_index < 0:
                    raise ValueError("could not add layer-assignment layer")
                layer_indices.append(layer_index)

            for index in range(5):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.LayerIndex = default_layer_index
                attributes.Name = "Part%d" % index
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0), attributes
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add layer-assignment point")
                object_ids.append(object_id)
            original_group_index = document.Groups.Add(
                "ViboLayerAssignmentAssembly" + suffix, object_ids[:2]
            )
            if original_group_index < 0:
                raise ValueError("could not add layer-assignment group")

            layer_labels = {
                default_layer_index: "Default",
                layer_indices[0]: "Normal",
                layer_indices[1]: "Hidden",
                layer_indices[2]: "Locked",
            }

            def point_ids():
                settings = Rhino.DocObjects.ObjectEnumeratorSettings()
                settings.NormalObjects = True
                settings.LockedObjects = True
                settings.HiddenObjects = True
                settings.ObjectTypeFilter = Rhino.DocObjects.ObjectType.Point
                return set(
                    obj.Id for obj in document.Objects.GetObjectList(settings)
                )

            def selected_indices(ids):
                return [
                    index
                    for index, object_id in enumerate(ids)
                    if document.Objects.FindId(object_id).IsSelected(False) != 0
                ]

            def select_only(ids):
                document.Objects.UnselectAll()
                for object_id in ids:
                    if not document.Objects.Select(object_id):
                        raise ValueError(
                            "could not preselect layer-assignment object"
                        )

            def object_layer_indices(ids):
                return [
                    int(document.Objects.FindId(object_id).Attributes.LayerIndex)
                    for object_id in ids
                ]

            def object_layer_labels(ids):
                labels = []
                for layer_index in object_layer_indices(ids):
                    if layer_index not in layer_labels:
                        raise ValueError(
                            "layer-assignment object used an unexpected layer"
                        )
                    labels.append(layer_labels[layer_index])
                return labels

            def group_indices_for(ids):
                ids = set(ids)
                result = []
                for group_index in range(document.Groups.Count):
                    members = document.Groups.GroupMembers(group_index)
                    if members is None:
                        continue
                    if any(member.Id in ids for member in members):
                        result.append(group_index)
                return result

            def group_sizes_for(ids):
                ids = set(ids)
                sizes = []
                for group_index in group_indices_for(ids):
                    members = document.Groups.GroupMembers(group_index)
                    size = sum(1 for member in members if member.Id in ids)
                    if size:
                        sizes.append(size)
                return sorted(sizes)

            def modify_object_layers(ids, layer_index):
                changed = 0
                for object_id in ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object is None:
                        raise ValueError("layer-assignment object disappeared")
                    if int(rhino_object.Attributes.LayerIndex) == layer_index:
                        continue
                    attributes = rhino_object.Attributes.Duplicate()
                    attributes.LayerIndex = layer_index
                    if not document.Objects.ModifyAttributes(
                        rhino_object, attributes, True
                    ):
                        raise ValueError(
                            "could not modify layer-assignment object"
                        )
                    changed += 1
                return changed

            def run_layer_command(command_name, layer_index):
                layer_name = document.Layers[layer_index].Name
                if not Rhino.RhinoApp.RunScript(
                    "_-%s %s _Enter" % (command_name, layer_name), False
                ):
                    raise ValueError(
                        "%s failed in layer-assignment cycle" % command_name
                    )

            def new_point_ids(before):
                ids = list(point_ids() - before)
                ids.sort(
                    key=lambda object_id: document.Objects.FindId(
                        object_id
                    ).Geometry.Location.X
                )
                return ids

            def reset_from_destination(ids, layer_index):
                layer = document.Layers[layer_index]
                visible = bool(layer.IsVisible)
                locked = bool(layer.IsLocked)
                if not visible or locked:
                    set_layer_mode(layer_index, True, False)
                modify_object_layers(ids, default_layer_index)
                if not visible or locked:
                    set_layer_mode(layer_index, visible, locked)

            def delete_copies(ids, layer_index):
                for group_index in reversed(group_indices_for(ids)):
                    document.Groups.Delete(group_index)
                layer = document.Layers[layer_index]
                visible = bool(layer.IsVisible)
                locked = bool(layer.IsLocked)
                if not visible or locked:
                    set_layer_mode(layer_index, True, False)
                for object_id in ids:
                    document.Objects.Show(object_id, True)
                    document.Objects.Unlock(object_id, True)
                    if not document.Objects.Delete(object_id, True):
                        raise ValueError("could not delete layer-assignment copy")
                if not visible or locked:
                    set_layer_mode(layer_index, visible, locked)

            current_before = int(document.Layers.CurrentLayerIndex)
            select_only(object_ids[:2])
            before_change_layers = object_layer_indices(object_ids[:2])
            run_layer_command("ChangeLayer", layer_indices[0])
            after_change_layers = object_layer_indices(object_ids[:2])
            change_count = sum(
                1
                for before, after in zip(
                    before_change_layers, after_change_layers
                )
                if before != after
            )
            change_layers = object_layer_labels(object_ids[:2])
            change_selected = selected_indices(object_ids)
            change_group_sizes = group_sizes_for(object_ids[:2])
            current_after_change = (
                "Default"
                if int(document.Layers.CurrentLayerIndex)
                == default_layer_index
                else "Unexpected"
            )
            reset_from_destination(object_ids[:2], layer_indices[0])

            before_copy = point_ids()
            select_only(object_ids[:2])
            run_layer_command("CopyToLayer", layer_indices[0])
            copy_ids = new_point_ids(before_copy)
            copy_count = len(copy_ids)
            copy_layers = object_layer_labels(copy_ids)
            copy_names = [
                document.Objects.FindId(object_id).Attributes.Name
                for object_id in copy_ids
            ]
            copy_group_sizes = group_sizes_for(copy_ids)
            copy_selected = selected_indices(copy_ids)
            original_selected_after_copy = selected_indices(object_ids)
            delete_copies(copy_ids, layer_indices[0])

            modify_object_layers([object_ids[0]], layer_indices[0])
            before_mixed_copy = point_ids()
            select_only(object_ids[:2])
            run_layer_command("CopyToLayer", layer_indices[0])
            mixed_copy_ids = new_point_ids(before_mixed_copy)
            mixed_copy_count = len(mixed_copy_ids)
            mixed_copy_layers = object_layer_labels(mixed_copy_ids)
            mixed_copy_group_sizes = group_sizes_for(mixed_copy_ids)
            delete_copies(mixed_copy_ids, layer_indices[0])
            reset_from_destination([object_ids[0]], layer_indices[0])

            modify_object_layers(object_ids[:2], layer_indices[0])
            before_same_layer_copy = point_ids()
            select_only(object_ids[:2])
            run_layer_command("CopyToLayer", layer_indices[0])
            same_layer_copy_ids = new_point_ids(before_same_layer_copy)
            same_layer_copy_count = len(same_layer_copy_ids)
            delete_copies(same_layer_copy_ids, layer_indices[0])
            reset_from_destination(object_ids[:2], layer_indices[0])

            select_only([object_ids[2]])
            before_hidden_change = object_layer_indices([object_ids[2]])
            run_layer_command("ChangeLayer", layer_indices[1])
            hidden_change_count = int(
                before_hidden_change
                != object_layer_indices([object_ids[2]])
            )
            hidden_change_selected = selected_indices(object_ids)
            reset_from_destination([object_ids[2]], layer_indices[1])

            select_only([object_ids[3]])
            before_locked_change = object_layer_indices([object_ids[3]])
            run_layer_command("ChangeLayer", layer_indices[2])
            locked_change_count = int(
                before_locked_change
                != object_layer_indices([object_ids[3]])
            )
            locked_change_selected = selected_indices(object_ids)
            reset_from_destination([object_ids[3]], layer_indices[2])

            before_hidden_copy = point_ids()
            select_only([object_ids[2]])
            run_layer_command("CopyToLayer", layer_indices[1])
            hidden_copy_ids = new_point_ids(before_hidden_copy)
            hidden_copy_count = len(hidden_copy_ids)
            hidden_copy_layers = object_layer_labels(hidden_copy_ids)
            hidden_copy_selected = selected_indices(hidden_copy_ids)
            delete_copies(hidden_copy_ids, layer_indices[1])

            before_locked_copy = point_ids()
            select_only([object_ids[3]])
            run_layer_command("CopyToLayer", layer_indices[2])
            locked_copy_ids = new_point_ids(before_locked_copy)
            locked_copy_count = len(locked_copy_ids)
            locked_copy_layers = object_layer_labels(locked_copy_ids)
            locked_copy_selected = selected_indices(locked_copy_ids)
            original_selected_after_destination_copies = selected_indices(
                object_ids
            )
            delete_copies(locked_copy_ids, layer_indices[2])

            value = {
                "change_count": change_count,
                "change_group_sizes": change_group_sizes,
                "change_layers": change_layers,
                "change_selected": change_selected,
                "copy_count": copy_count,
                "copy_group_sizes": copy_group_sizes,
                "copy_layers": copy_layers,
                "copy_names": copy_names,
                "copy_selected": copy_selected,
                "current_after_change": current_after_change,
                "current_unchanged": (
                    int(document.Layers.CurrentLayerIndex) == current_before
                ),
                "hidden_change_count": hidden_change_count,
                "hidden_change_selected": hidden_change_selected,
                "hidden_copy_count": hidden_copy_count,
                "hidden_copy_layers": hidden_copy_layers,
                "hidden_copy_selected": hidden_copy_selected,
                "locked_change_count": locked_change_count,
                "locked_change_selected": locked_change_selected,
                "locked_copy_count": locked_copy_count,
                "locked_copy_layers": locked_copy_layers,
                "locked_copy_selected": locked_copy_selected,
                "mixed_copy_count": mixed_copy_count,
                "mixed_copy_group_sizes": mixed_copy_group_sizes,
                "mixed_copy_layers": mixed_copy_layers,
                "original_selected_after_copy": original_selected_after_copy,
                "original_selected_after_destination_copies": (
                    original_selected_after_destination_copies
                ),
                "same_layer_copy_count": same_layer_copy_count,
            }

            modify_object_layers(object_ids[:2], layer_indices[0])
            modify_object_layers(object_ids[:2], default_layer_index)
            started = default_timer()
            for _unused in iteration_range(iterations):
                modify_object_layers(object_ids[:2], layer_indices[0])
                modify_object_layers(object_ids[:2], default_layer_index)
            elapsed_ns = int(
                round((default_timer() - started) * 1000000000.0)
            )
            return value, max(0, elapsed_ns)
        finally:
            document.Objects.UnselectAll()
            for layer_index in layer_indices:
                layer = document.Layers[layer_index]
                if layer is not None and (
                    not layer.IsVisible or layer.IsLocked
                ):
                    set_layer_mode(layer_index, True, False)
            if original_group_index is not None and original_group_index >= 0:
                document.Groups.Delete(original_group_index)
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)
            for layer_index in reversed(layer_indices):
                document.Layers.Delete(layer_index, True)
    if kind == "document_object_state_cycle":
        object_count_value = operation.get("object_count")
        if (
            isinstance(object_count_value, bool)
            or int(object_count_value) != object_count_value
        ):
            raise ValueError("object_count must be an integer")
        object_count = int(object_count_value)
        if object_count < 1 or object_count > MAX_STATE_CYCLE_OBJECTS:
            raise ValueError(
                "object_count must be from 1 through %d"
                % MAX_STATE_CYCLE_OBJECTS
            )
        hide_indices = _state_cycle_indices(
            operation, "hide_indices", object_count
        )
        lock_indices = _state_cycle_indices(
            operation, "lock_indices", object_count
        )
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        try:
            for index in range(object_count):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document state-cycle point")
                object_ids.append(object_id)

            hide_ids = [object_ids[index] for index in hide_indices]
            lock_ids = [object_ids[index] for index in lock_indices]

            def object_modes():
                modes = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object is None:
                        raise ValueError("document state-cycle object disappeared")
                    if rhino_object.IsHidden:
                        modes.append("hidden")
                    elif rhino_object.IsLocked:
                        modes.append("locked")
                    else:
                        modes.append("normal")
                return modes

            def state_cycle():
                document.Objects.UnselectAll()
                for object_id in hide_ids:
                    if not document.Objects.Select(object_id):
                        raise ValueError("could not select object to hide")
                hide_count = sum(
                    1
                    for object_id in hide_ids
                    if document.Objects.Hide(object_id, True)
                )
                modes_after_hide = object_modes()
                selected_after_hide = int(
                    document.Objects.GetSelectedObjectCount(False)
                )

                show_count = sum(
                    1
                    for object_id in hide_ids
                    if document.Objects.Show(object_id, True)
                )
                modes_after_show = object_modes()
                for object_id in lock_ids:
                    if not document.Objects.Select(object_id):
                        raise ValueError("could not select object to lock")
                lock_count = sum(
                    1
                    for object_id in lock_ids
                    if document.Objects.Lock(object_id, True)
                )
                modes_after_lock = object_modes()
                selected_after_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )

                unlock_count = sum(
                    1
                    for object_id in lock_ids
                    if document.Objects.Unlock(object_id, True)
                )
                modes_after_unlock = object_modes()
                return {
                    "hide_count": hide_count,
                    "lock_count": lock_count,
                    "modes_after_hide": modes_after_hide,
                    "modes_after_lock": modes_after_lock,
                    "modes_after_show": modes_after_show,
                    "modes_after_unlock": modes_after_unlock,
                    "selected_after_hide": selected_after_hide,
                    "selected_after_lock": selected_after_lock,
                    "show_count": show_count,
                    "unlock_count": unlock_count,
                }

            return _measure(iterations, state_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_object_swap_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        try:
            for index in range(3):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document swap-cycle point")
                object_ids.append(object_id)
            if not document.Objects.Hide(object_ids[1], True):
                raise ValueError("could not seed hidden object")
            if not document.Objects.Lock(object_ids[2], True):
                raise ValueError("could not seed locked object")

            layer_suffix = str(System.Guid.NewGuid())
            hidden_layer = Rhino.DocObjects.Layer()
            hidden_layer.Name = "Viboceros Swap Cycle Hidden " + layer_suffix
            hidden_layer.IsVisible = False
            hidden_layer_index = document.Layers.Add(hidden_layer)
            locked_layer = Rhino.DocObjects.Layer()
            locked_layer.Name = "Viboceros Swap Cycle Locked " + layer_suffix
            locked_layer.IsLocked = True
            locked_layer_index = document.Layers.Add(locked_layer)
            if hidden_layer_index < 0 or locked_layer_index < 0:
                raise ValueError("could not add document swap-cycle layers")
            for layer_index in (hidden_layer_index, locked_layer_index):
                for mode_index in range(3):
                    attributes = Rhino.DocObjects.ObjectAttributes()
                    attributes.LayerIndex = layer_index
                    object_id = document.Objects.AddPoint(
                        Rhino.Geometry.Point3d(float(len(object_ids)), 0.0, 0.0),
                        attributes,
                    )
                    if object_id == System.Guid.Empty:
                        raise ValueError("could not add layered swap-cycle point")
                    object_ids.append(object_id)
                    if mode_index == 1 and not document.Objects.Hide(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered hidden object")
                    if mode_index == 2 and not document.Objects.Lock(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered locked object")

            def swap_modes():
                result = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object.IsHidden:
                        result.append("hidden")
                    elif rhino_object.IsLocked:
                        result.append("locked")
                    else:
                        result.append("normal")
                return result

            def layer_allows_swap(rhino_object):
                layer = document.Layers[rhino_object.Attributes.LayerIndex]
                return bool(layer.IsVisible and not layer.IsLocked)

            def hide_swap():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if not layer_allows_swap(rhino_object):
                        continue
                    if rhino_object.IsHidden:
                        changed += int(document.Objects.Show(object_id, True))
                    elif not rhino_object.IsLocked:
                        changed += int(document.Objects.Hide(object_id, True))
                return changed

            def lock_swap():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if not layer_allows_swap(rhino_object) or rhino_object.IsHidden:
                        continue
                    if rhino_object.IsLocked:
                        changed += int(document.Objects.Unlock(object_id, True))
                    else:
                        changed += int(document.Objects.Lock(object_id, True))
                return changed

            labels = [
                "default-normal",
                "default-hidden",
                "default-locked",
                "hidden-layer-normal",
                "hidden-layer-hidden",
                "hidden-layer-locked",
                "locked-layer-normal",
                "locked-layer-hidden",
                "locked-layer-locked",
            ]

            def swap_cycle():
                document.Objects.UnselectAll()
                if not document.Objects.Select(object_ids[0]):
                    raise ValueError("could not select object before HideSwap")
                hide_count_once = hide_swap()
                hide_once = swap_modes()
                selected_after_hide = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                hide_count_twice = hide_swap()
                hide_twice = swap_modes()

                if not document.Objects.Select(object_ids[0]):
                    raise ValueError("could not select object before LockSwap")
                lock_count_once = lock_swap()
                lock_once = swap_modes()
                selected_after_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                lock_count_twice = lock_swap()
                return {
                    "hide_count_once": hide_count_once,
                    "hide_count_twice": hide_count_twice,
                    "hide_once": hide_once,
                    "hide_twice": hide_twice,
                    "labels": labels,
                    "lock_count_once": lock_count_once,
                    "lock_count_twice": lock_count_twice,
                    "lock_once": lock_once,
                    "lock_twice": swap_modes(),
                    "selected_after_hide": selected_after_hide,
                    "selected_after_lock": selected_after_lock,
                }

            return _measure(iterations, swap_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_object_isolation_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        isolated_hidden = []
        isolated_locked = []
        try:
            for index in range(4):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document isolation-cycle point")
                object_ids.append(object_id)
            if not document.Objects.Hide(object_ids[2], True):
                raise ValueError("could not seed isolation-cycle hidden object")
            if not document.Objects.Lock(object_ids[3], True):
                raise ValueError("could not seed isolation-cycle locked object")

            layer_suffix = str(System.Guid.NewGuid())
            hidden_layer = Rhino.DocObjects.Layer()
            hidden_layer.Name = "Viboceros Isolation Cycle Hidden " + layer_suffix
            hidden_layer.IsVisible = False
            hidden_layer_index = document.Layers.Add(hidden_layer)
            locked_layer = Rhino.DocObjects.Layer()
            locked_layer.Name = "Viboceros Isolation Cycle Locked " + layer_suffix
            locked_layer.IsLocked = True
            locked_layer_index = document.Layers.Add(locked_layer)
            if hidden_layer_index < 0 or locked_layer_index < 0:
                raise ValueError("could not add document isolation-cycle layers")
            for layer_index in (hidden_layer_index, locked_layer_index):
                for mode_index in range(3):
                    attributes = Rhino.DocObjects.ObjectAttributes()
                    attributes.LayerIndex = layer_index
                    object_id = document.Objects.AddPoint(
                        Rhino.Geometry.Point3d(float(len(object_ids)), 0.0, 0.0),
                        attributes,
                    )
                    if object_id == System.Guid.Empty:
                        raise ValueError(
                            "could not add layered isolation-cycle point"
                        )
                    object_ids.append(object_id)
                    if mode_index == 1 and not document.Objects.Hide(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered hidden object")
                    if mode_index == 2 and not document.Objects.Lock(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered locked object")

            def isolation_modes():
                result = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object.IsHidden:
                        result.append("hidden")
                    elif rhino_object.IsLocked:
                        result.append("locked")
                    else:
                        result.append("normal")
                return result

            def layer_allows_isolation(rhino_object):
                layer = document.Layers[rhino_object.Attributes.LayerIndex]
                return bool(layer.IsVisible and not layer.IsLocked)

            def isolate():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if (
                        rhino_object.IsSelected(False) != 0
                        or rhino_object.IsHidden
                        or rhino_object.IsLocked
                        or not layer_allows_isolation(rhino_object)
                    ):
                        continue
                    if document.Objects.Hide(object_id, True):
                        isolated_hidden.append(object_id)
                        changed += 1
                return changed

            def unisolate():
                changed = sum(
                    1
                    for object_id in isolated_hidden
                    if document.Objects.Show(object_id, True)
                )
                del isolated_hidden[:]
                return changed

            def isolate_lock():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if (
                        rhino_object.IsSelected(False) != 0
                        or rhino_object.IsHidden
                        or rhino_object.IsLocked
                        or not layer_allows_isolation(rhino_object)
                    ):
                        continue
                    if document.Objects.Lock(object_id, True):
                        isolated_locked.append(object_id)
                        changed += 1
                return changed

            def unisolate_lock():
                changed = sum(
                    1
                    for object_id in isolated_locked
                    if document.Objects.Unlock(object_id, True)
                )
                del isolated_locked[:]
                return changed

            labels = [
                "default-selected",
                "default-normal",
                "default-hidden",
                "default-locked",
                "hidden-layer-normal",
                "hidden-layer-hidden",
                "hidden-layer-locked",
                "locked-layer-normal",
                "locked-layer-hidden",
                "locked-layer-locked",
            ]

            def isolation_cycle():
                document.Objects.UnselectAll()
                if not document.Objects.Select(object_ids[0]):
                    raise ValueError("could not select isolation survivor")
                isolate_count = isolate()
                isolate_repeat_count = isolate()
                after_isolate = isolation_modes()
                selected_after_isolate = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                unisolate_count = unisolate()
                unisolate_repeat_count = unisolate()
                after_unisolate = isolation_modes()
                selected_after_unisolate = int(
                    document.Objects.GetSelectedObjectCount(False)
                )

                if (
                    document.Objects.FindId(object_ids[0]).IsSelected(False) == 0
                    and not document.Objects.Select(object_ids[0])
                ):
                    raise ValueError("could not select isolation-lock survivor")
                isolate_lock_count = isolate_lock()
                isolate_lock_repeat_count = isolate_lock()
                after_isolate_lock = isolation_modes()
                selected_after_isolate_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                unisolate_lock_count = unisolate_lock()
                unisolate_lock_repeat_count = unisolate_lock()
                selected_after_unisolate_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                return {
                    "after_isolate": after_isolate,
                    "after_isolate_lock": after_isolate_lock,
                    "after_unisolate": after_unisolate,
                    "after_unisolate_lock": isolation_modes(),
                    "isolate_count": isolate_count,
                    "isolate_lock_count": isolate_lock_count,
                    "isolate_lock_repeat_count": isolate_lock_repeat_count,
                    "isolate_repeat_count": isolate_repeat_count,
                    "labels": labels,
                    "selected_after_isolate": selected_after_isolate,
                    "selected_after_isolate_lock": selected_after_isolate_lock,
                    "selected_after_unisolate": selected_after_unisolate,
                    "selected_after_unisolate_lock": selected_after_unisolate_lock,
                    "unisolate_count": unisolate_count,
                    "unisolate_lock_count": unisolate_lock_count,
                    "unisolate_lock_repeat_count": unisolate_lock_repeat_count,
                    "unisolate_repeat_count": unisolate_repeat_count,
                }

            return _measure(iterations, isolation_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_action_selection_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        batch_ids = []
        state = {"previous": []}
        try:
            for index in range(4):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add action-selection point")
                object_ids.append(object_id)
            for index in range(2):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 1.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add batch action-selection point")
                batch_ids.append(object_id)
            all_ids = object_ids + batch_ids
            last_changed = [object_ids[3]]
            batch_last_changed = list(batch_ids)

            def current_selection():
                return [
                    object_id
                    for object_id in all_ids
                    if document.Objects.FindId(object_id).IsSelected(False) != 0
                ]

            def assign_selection(ids):
                ids = set(ids)
                document.Objects.UnselectAll()
                for object_id in all_ids:
                    if object_id in ids and not document.Objects.Select(object_id):
                        raise ValueError("could not select action-selection object")

            def apply_selection(ids, deselect_others):
                current = current_selection()
                target = set(ids)
                if deselect_others:
                    next_selection = [
                        object_id for object_id in all_ids if object_id in target
                    ]
                else:
                    combined = set(current)
                    combined.update(target)
                    next_selection = [
                        object_id for object_id in all_ids if object_id in combined
                    ]
                if not set(current).issubset(set(next_selection)):
                    state["previous"] = current
                assign_selection(next_selection)
                return len(next_selection)

            def select_previous(deselect_others):
                current = current_selection()
                previous = set(state["previous"])
                if deselect_others:
                    next_selection = [
                        object_id for object_id in all_ids if object_id in previous
                    ]
                else:
                    combined = set(current)
                    combined.update(previous)
                    next_selection = [
                        object_id for object_id in all_ids if object_id in combined
                    ]
                assign_selection(next_selection)
                state["previous"] = current
                return len(next_selection)

            def selected_indices(ids):
                return [
                    index
                    for index, object_id in enumerate(ids)
                    if document.Objects.FindId(object_id).IsSelected(False) != 0
                ]

            def establish_previous():
                apply_selection([object_ids[0], object_ids[1]], True)
                apply_selection([], True)
                apply_selection([object_ids[2]], False)

            def action_selection_cycle():
                apply_selection([], True)
                apply_selection([object_ids[0]], True)
                last_default_count = apply_selection(last_changed, True)
                last_default = selected_indices(object_ids)
                previous_once_count = select_previous(True)
                previous_once = selected_indices(object_ids)
                previous_twice_count = select_previous(True)
                previous_twice = selected_indices(object_ids)

                apply_selection([object_ids[0]], True)
                last_add_count = apply_selection(last_changed, False)
                last_add = selected_indices(object_ids)

                establish_previous()
                previous_default_count = select_previous(True)
                previous_default = selected_indices(object_ids)
                previous_default_twice_count = select_previous(True)
                previous_default_twice = selected_indices(object_ids)

                establish_previous()
                previous_add_count = select_previous(False)
                previous_add = selected_indices(object_ids)

                apply_selection([], True)
                batch_last_count = apply_selection(batch_last_changed, True)
                return {
                    "batch_last": selected_indices(batch_ids),
                    "batch_last_count": batch_last_count,
                    "last_add": last_add,
                    "last_add_count": last_add_count,
                    "last_default": last_default,
                    "last_default_count": last_default_count,
                    "previous_add": previous_add,
                    "previous_add_count": previous_add_count,
                    "previous_default": previous_default,
                    "previous_default_count": previous_default_count,
                    "previous_default_twice": previous_default_twice,
                    "previous_default_twice_count": previous_default_twice_count,
                    "previous_once": previous_once,
                    "previous_once_count": previous_once_count,
                    "previous_twice": previous_twice,
                    "previous_twice_count": previous_twice_count,
                }

            return _measure(iterations, action_selection_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids + batch_ids:
                document.Objects.Delete(object_id, True)

    if kind == "document_attribute_selection_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        group_indices = []
        suffix = " " + str(System.Guid.NewGuid())
        hidden_layer = Rhino.DocObjects.Layer()
        hidden_layer.Name = "Hidden Parts" + suffix
        hidden_layer_index = document.Layers.Add(hidden_layer)
        locked_layer = Rhino.DocObjects.Layer()
        locked_layer.Name = "Locked Parts" + suffix
        locked_layer_index = document.Layers.Add(locked_layer)
        if hidden_layer_index < 0 or locked_layer_index < 0:
            raise ValueError("could not add attribute-selection layers")
        default_layer_index = document.Layers.CurrentLayerIndex
        probe_layer_indices = [
            default_layer_index,
            hidden_layer_index,
            locked_layer_index,
        ]

        def set_layer_mode(layer_index, visible, locked):
            layer = document.Layers[layer_index]
            if layer.IsVisible == visible and layer.IsLocked == locked:
                return
            settings = Rhino.DocObjects.Layer()
            settings.CopyAttributesFrom(layer)
            settings.Name = layer.Name
            settings.ParentLayerId = layer.ParentLayerId
            settings.IsVisible = visible
            settings.IsLocked = locked
            if not document.Layers.Modify(settings, layer.Id, True):
                raise ValueError(
                    "could not change attribute-selection layer %d to visible=%r locked=%r"
                    % (layer_index, visible, locked)
                )

        try:
            specifications = [
                (default_layer_index, None),
                (default_layer_index, "BoltA"),
                (default_layer_index, "bolta"),
                (default_layer_index, "BoltLong"),
                (default_layer_index, "Peer"),
                (default_layer_index, "BoltA"),
                (default_layer_index, "BoltA"),
                (hidden_layer_index, "BoltA"),
                (hidden_layer_index, "BoltA"),
                (locked_layer_index, "BoltA"),
                (locked_layer_index, "BoltA"),
            ]
            for index, (layer_index, name) in enumerate(specifications):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.LayerIndex = layer_index
                attributes.Name = name
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0), attributes
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add attribute-selection point")
                object_ids.append(object_id)
            for index in (5, 8):
                if not document.Objects.Hide(object_ids[index], True):
                    raise ValueError("could not hide attribute-selection point")
            for index in (6, 10):
                if not document.Objects.Lock(object_ids[index], True):
                    raise ValueError("could not lock attribute-selection point")

            for name, members in (
                ("Team" + suffix, [object_ids[1], object_ids[4], object_ids[6]]),
                ("team" + suffix, [object_ids[2]]),
                ("Overlap" + suffix, [object_ids[1], object_ids[3]]),
            ):
                group_index = document.Groups.Add(
                    name, System.Array[System.Guid](members)
                )
                if group_index < 0:
                    raise ValueError("could not add attribute-selection group")
                group_indices.append(group_index)

            def object_is_selectable(object_id):
                rhino_object = document.Objects.FindId(object_id)
                if (
                    rhino_object is None
                    or rhino_object.IsHidden
                    or rhino_object.IsLocked
                ):
                    return False
                layer = document.Layers[rhino_object.Attributes.LayerIndex]
                return bool(layer.IsVisible and not layer.IsLocked)

            def selected_indices():
                return [
                    index
                    for index, object_id in enumerate(object_ids)
                    if document.Objects.FindId(object_id).IsSelected(False) > 0
                ]

            def select_ids(ids):
                for object_id in ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if (
                        object_is_selectable(object_id)
                        and rhino_object.IsSelected(False) <= 0
                        and not document.Objects.Select(object_id)
                    ):
                        raise ValueError("could not select attribute-selection point")
                return int(document.Objects.GetSelectedObjectCount(False))

            def select_name(pattern):
                matches = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    name = rhino_object.Attributes.Name or ""
                    if _wildcard_matches(pattern, name):
                        matches.append(object_id)
                return select_ids(matches)

            def select_group(name):
                matches = []
                for group_index in group_indices:
                    if document.Groups.GroupName(group_index) != name:
                        continue
                    members = document.Groups.GroupMembers(group_index)
                    if members is not None:
                        matches.extend(member.Id for member in members)
                return select_ids(matches)

            def select_layers(pattern):
                matching_layers = []
                for layer_index in probe_layer_indices:
                    layer = document.Layers[layer_index]
                    if _wildcard_matches(pattern, layer.Name):
                        matching_layers.append(layer_index)
                        set_layer_mode(layer_index, True, False)
                return select_ids(
                    object_id
                    for object_id in object_ids
                    if document.Objects.FindId(object_id).Attributes.LayerIndex
                    in matching_layers
                )

            def seed_selection():
                document.Objects.UnselectAll()
                return select_ids([object_ids[0]])

            def attribute_selection_cycle():
                set_layer_mode(hidden_layer_index, False, False)
                set_layer_mode(locked_layer_index, True, True)

                seed_selection()
                name_count = select_name("BOLT?")
                name = selected_indices()

                seed_selection()
                group_upper_count = select_group("Team" + suffix)
                group_upper = selected_indices()
                group_lower_count = select_group("team" + suffix)
                group_lower = selected_indices()
                group_wrong_case_count = select_group("TEAM" + suffix)
                group_wrong_case = selected_indices()

                seed_selection()
                hidden_layer_count = select_layers("hidden parts*")
                hidden_layer_selection = selected_indices()
                hidden_layer_visible = bool(
                    document.Layers[hidden_layer_index].IsVisible
                )
                locked_layer_count = select_layers("LOCKED*")
                locked_layer_selection = selected_indices()
                locked_layer_locked = bool(
                    document.Layers[locked_layer_index].IsLocked
                )
                all_layers_count = select_layers("*")
                return {
                    "all_layers": selected_indices(),
                    "all_layers_count": all_layers_count,
                    "group_lower": group_lower,
                    "group_lower_count": group_lower_count,
                    "group_upper": group_upper,
                    "group_upper_count": group_upper_count,
                    "group_wrong_case": group_wrong_case,
                    "group_wrong_case_count": group_wrong_case_count,
                    "hidden_layer": hidden_layer_selection,
                    "hidden_layer_count": hidden_layer_count,
                    "hidden_layer_visible": hidden_layer_visible,
                    "locked_layer": locked_layer_selection,
                    "locked_layer_count": locked_layer_count,
                    "locked_layer_locked": locked_layer_locked,
                    "name": name,
                    "name_count": name_count,
                }

            return _measure(iterations, attribute_selection_cycle)
        finally:
            document.Objects.UnselectAll()
            for group_index in reversed(group_indices):
                document.Groups.Delete(group_index)
            set_layer_mode(hidden_layer_index, True, False)
            set_layer_mode(locked_layer_index, True, False)
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_object_naming_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        try:
            for index in range(3):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document naming-cycle point")
                object_ids.append(object_id)

            def object_names():
                return [
                    document.Objects.FindId(object_id).Attributes.Name
                    for object_id in object_ids
                ]

            def assign_names(names):
                changed = 0
                for object_id, name in zip(object_ids, names):
                    rhino_object = document.Objects.FindId(object_id)
                    attributes = rhino_object.Attributes.Duplicate()
                    attributes.Name = name
                    changed += int(
                        document.Objects.ModifyAttributes(
                            rhino_object, attributes, True
                        )
                    )
                return changed

            def naming_cycle():
                shared_count = assign_names(["Sample", "Sample", "Sample"])
                shared = object_names()
                counter_count = assign_names(
                    ["Sample 0", "Sample 1", "Sample 2"]
                )
                counter = object_names()
                clear_count = assign_names([None, None, None])
                return {
                    "clear_count": clear_count,
                    "cleared": object_names(),
                    "counter": counter,
                    "counter_count": counter_count,
                    "shared": shared,
                    "shared_count": shared_count,
                }

            return _measure(iterations, naming_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Delete(object_id, True)

    if kind == "document_duplicate_selection_cycle":
        geometries = []
        selectable = []

        def remember(geometry, is_selectable=True):
            if geometry is None or not geometry.IsValid:
                raise ValueError("could not create duplicate-selection geometry")
            geometries.append(geometry)
            selectable.append(bool(is_selectable))
            return len(geometries) - 1

        try:
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(30.0, 0.0, 0.0)))
            point_original = remember(
                Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0))
            )
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)))
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)))
            remember(
                Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)),
                False,
            )
            remember(
                Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)),
                False,
            )
            point_near = remember(
                Rhino.Geometry.Point(
                    Rhino.Geometry.Point3d(
                        0.5 * tolerance["absolute"], 0.0, 0.0
                    )
                )
            )
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(20.0, 0.0, 0.0)))

            line_start = Rhino.Geometry.Point3d(0.0, 10.0, 0.0)
            line_end = Rhino.Geometry.Point3d(5.0, 10.0, 0.0)
            line_original = remember(Rhino.Geometry.LineCurve(line_start, line_end))
            remember(Rhino.Geometry.LineCurve(line_start, line_end))
            line_reversed = remember(Rhino.Geometry.LineCurve(line_end, line_start))
            line_nurbs_geometry = Rhino.Geometry.NurbsCurve.Create(
                False,
                1,
                System.Array[Rhino.Geometry.Point3d]([line_start, line_end]),
            )
            line_nurbs = remember(line_nurbs_geometry)
            line_near = remember(
                Rhino.Geometry.LineCurve(
                    line_start,
                    Rhino.Geometry.Point3d(
                        5.0 + 0.5 * tolerance["absolute"], 10.0, 0.0
                    ),
                )
            )

            open_vertices = [
                Rhino.Geometry.Point3d(0.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(2.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(2.0, 22.0, 0.0),
            ]
            open_polyline = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](open_vertices)
                )
            )
            remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](open_vertices)
                )
            )
            open_polyline_reversed = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](list(reversed(open_vertices)))
                )
            )

            closed_vertices = [
                Rhino.Geometry.Point3d(10.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(12.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(12.0, 22.0, 0.0),
                Rhino.Geometry.Point3d(10.0, 20.0, 0.0),
            ]
            closed_polyline = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](closed_vertices)
                )
            )
            shifted_closed_polyline = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](
                        [
                            closed_vertices[1],
                            closed_vertices[2],
                            closed_vertices[0],
                            closed_vertices[1],
                        ]
                    )
                )
            )

            circle_center = Rhino.Geometry.Point3d(0.0, 30.0, 0.0)
            circle_plane = Rhino.Geometry.Plane(
                circle_center,
                Rhino.Geometry.Vector3d.XAxis,
                Rhino.Geometry.Vector3d.YAxis,
            )
            opposite_circle_plane = Rhino.Geometry.Plane(
                circle_center,
                Rhino.Geometry.Vector3d.XAxis,
                -Rhino.Geometry.Vector3d.YAxis,
            )
            circle_original = remember(
                Rhino.Geometry.ArcCurve(Rhino.Geometry.Circle(circle_plane, 3.0))
            )
            remember(Rhino.Geometry.ArcCurve(Rhino.Geometry.Circle(circle_plane, 3.0)))
            circle_opposite = remember(
                Rhino.Geometry.ArcCurve(
                    Rhino.Geometry.Circle(opposite_circle_plane, 3.0)
                )
            )

            mesh_vertices = [
                [0.0, 40.0, 0.0],
                [2.0, 40.0, 0.0],
                [0.0, 42.0, 0.0],
            ]
            mesh_original = remember(
                _triangle_mesh(mesh_vertices, [[0, 1, 2]])
            )
            remember(_triangle_mesh(mesh_vertices, [[0, 1, 2]]))
            mesh_reversed = remember(
                _triangle_mesh(mesh_vertices, [[0, 2, 1]])
            )
            mesh_reindexed = remember(
                _triangle_mesh(
                    [mesh_vertices[1], mesh_vertices[2], mesh_vertices[0]],
                    [[2, 0, 1]],
                )
            )

            def geometry_equal(left, right):
                return bool(
                    Rhino.Geometry.GeometryBase.GeometryEquals(
                        geometries[left], geometries[right]
                    )
                )

            def duplicate_classes():
                classes = []
                for index in range(len(geometries)):
                    if not selectable[index]:
                        continue
                    matching = None
                    for candidate_class in classes:
                        if geometry_equal(index, candidate_class[0]):
                            matching = candidate_class
                            break
                    if matching is None:
                        classes.append([index])
                    else:
                        matching.append(index)
                return [candidate_class for candidate_class in classes if len(candidate_class) > 1]

            def duplicate_selection_cycle():
                classes = duplicate_classes()
                selected_all = set([0])
                selected_without_originals = set([0])
                for candidate_class in classes:
                    selected_all.update(candidate_class)
                    selected_without_originals.update(candidate_class[1:])
                return {
                    "all": sorted(selected_all),
                    "all_count": len(selected_all),
                    "circle_opposite_equal": geometry_equal(
                        circle_original, circle_opposite
                    ),
                    "closed_shifted_equal": geometry_equal(
                        closed_polyline, shifted_closed_polyline
                    ),
                    "line_nurbs_equal": geometry_equal(
                        line_original, line_nurbs
                    ),
                    "line_near_equal": geometry_equal(line_original, line_near),
                    "line_reversed_equal": geometry_equal(
                        line_original, line_reversed
                    ),
                    "mesh_reindexed_equal": geometry_equal(
                        mesh_original, mesh_reindexed
                    ),
                    "mesh_reversed_equal": geometry_equal(
                        mesh_original, mesh_reversed
                    ),
                    "point_near_equal": geometry_equal(
                        point_original, point_near
                    ),
                    "polyline_reversed_equal": geometry_equal(
                        open_polyline, open_polyline_reversed
                    ),
                    "without_original_count": len(selected_without_originals),
                }

            return _measure(iterations, duplicate_selection_cycle)
        finally:
            for geometry in geometries:
                geometry.Dispose()

    if kind == "point_distance":
        a = _point(operation["a"])
        b = _point(operation["b"])
        value, elapsed = _measure(iterations, lambda: a.DistanceTo(b))
        return float(value), elapsed

    if kind == "line_point":
        line = Rhino.Geometry.Line(
            _point(operation["start"]), _point(operation["end"])
        )
        if not line.IsValid:
            raise ValueError("line is degenerate")
        parameter = _finite(operation["parameter"], "line parameter")
        value, elapsed = _measure(iterations, lambda: line.PointAt(parameter))
        return _xyz(value), elapsed

    if kind == "circle_point":
        center = _point(operation["center"])
        normal = _unit(
            _vector(operation["normal"]), tolerance["absolute"], "circle normal"
        )
        x_axis = _unit(
            _vector(operation["x_axis"]), tolerance["absolute"], "circle x axis"
        )
        projection = x_axis - normal * (x_axis * normal)
        projected_length = float(projection.Length)
        x_axis = _unit(projection, tolerance["angular"], "circle x axis")
        y_axis = Rhino.Geometry.Vector3d.CrossProduct(normal, x_axis)
        radius = _finite(operation["radius"], "circle radius") * projected_length
        if not radius > tolerance["absolute"]:
            raise ValueError("circle radius is degenerate")
        plane = Rhino.Geometry.Plane(center, x_axis, y_axis)
        circle = Rhino.Geometry.Circle(plane, radius)
        angle = _finite(operation["angle_radians"], "circle angle")
        value, elapsed = _measure(iterations, lambda: circle.PointAt(angle))
        return _xyz(value), elapsed

    if kind == "arc_three_point":
        arc = Rhino.Geometry.Arc(
            _point(operation["start"]),
            _point(operation["through"]),
            _point(operation["end"]),
        )
        if not arc.IsValid:
            raise ValueError("three-point arc is degenerate")
        normalized = _finite(
            operation["normalized_parameter"], "normalized arc parameter"
        )
        if normalized < 0.0 or normalized > 1.0:
            raise ValueError("normalized arc parameter is outside [0, 1]")
        parameter = arc.AngleDomain.ParameterAt(normalized)
        value, elapsed = _measure(iterations, lambda: arc.PointAt(parameter))
        return {
            "center": _xyz(arc.Center),
            "point": _xyz(value),
            "radius": float(arc.Radius),
            "sweep_radians": float(arc.Angle),
        }, elapsed

    if kind == "ellipse_three_point":
        ellipse = Rhino.Geometry.Ellipse(
            _point(operation["center"]),
            _point(operation["first_axis_point"]),
            _point(operation["second_axis_point"]),
        )
        if not ellipse.IsValid:
            raise ValueError("three-point ellipse is degenerate")
        angle = _finite(operation["angle_radians"], "ellipse angle")
        plane = ellipse.Plane

        def ellipse_point():
            return plane.PointAt(
                ellipse.Radius1 * math.cos(angle),
                ellipse.Radius2 * math.sin(angle),
            )

        value, elapsed = _measure(iterations, ellipse_point)
        return {
            "center": _xyz(plane.Origin),
            "point": _xyz(value),
            "radius_x": float(ellipse.Radius1),
            "radius_y": float(ellipse.Radius2),
            "x_axis": _xyz(plane.XAxis),
            "y_axis": _xyz(plane.YAxis),
        }, elapsed

    if kind == "polyline_length":
        polyline = Rhino.Geometry.Polyline(
            [_point(vertex) for vertex in operation["vertices"]]
        )
        if not polyline.IsValid:
            raise ValueError("polyline is invalid")
        value, elapsed = _measure(iterations, lambda: polyline.Length)
        return float(value), elapsed

    if kind == "polyline_area":
        polyline = Rhino.Geometry.Polyline(
            [_point(vertex) for vertex in operation["vertices"]]
        )
        if not polyline.IsValid or not polyline.IsClosed:
            raise ValueError("area polyline must be valid and closed")
        curve = Rhino.Geometry.PolylineCurve(polyline)

        def polyline_area():
            properties = Rhino.Geometry.AreaMassProperties.Compute(curve)
            if properties is None:
                raise ValueError("could not compute polyline area")
            area = float(properties.Area)
            properties.Dispose()
            return area

        value, elapsed = _measure(iterations, polyline_area)
        return value, elapsed

    if kind == "polyline_join":
        curves = []
        for vertices in operation["polylines"]:
            polyline = Rhino.Geometry.Polyline(
                [_point(vertex) for vertex in vertices]
            )
            if not polyline.IsValid:
                raise ValueError("polyline to join is invalid")
            curves.append(Rhino.Geometry.PolylineCurve(polyline))

        def join_curves():
            return Rhino.Geometry.Curve.JoinCurves(
                curves, tolerance["absolute"], False
            )

        value, elapsed = _measure(iterations, join_curves)
        return _canonical_join_segments(value), elapsed

    if kind == "nurbs_curve_evaluate":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        parameter = _finite(operation["parameter"], "curve parameter")
        value, elapsed = _measure(
            iterations, lambda: curve.DerivativeAt(parameter, 1)
        )
        if value is None or len(value) < 2:
            raise ValueError("NURBS curve evaluation failed")
        return {"point": _xyz(value[0]), "derivative": _xyz(value[1])}, elapsed

    if kind == "nurbs_curve_length":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        value, elapsed = _measure(
            iterations, lambda: curve.GetLength(tolerance["relative"])
        )
        return float(value), elapsed

    if kind == "nurbs_curve_short_filter":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        maximum_length = _finite(
            operation["maximum_length"], "maximum curve length"
        )
        if not maximum_length > 0.0:
            raise ValueError("maximum curve length must be strictly positive")
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")

        def curve_short_filter():
            return bool(
                curve.GetLength(tolerance["relative"]) <= maximum_length
            )

        return _measure(iterations, curve_short_filter)

    if kind == "nurbs_curve_topology":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")

        def curve_topology():
            return {
                "is_closed": bool(curve.IsClosed),
                "is_periodic": bool(curve.IsPeriodic),
            }

        return _measure(iterations, curve_topology)

    if kind == "nurbs_curve_classification":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")

        def curve_classification():
            is_linear_zero = bool(curve.IsLinear())
            return {
                "is_linear_model": bool(
                    curve.IsLinear(tolerance["absolute"])
                ),
                "is_linear_zero": is_linear_zero,
                "is_planar_model": bool(
                    curve.IsPlanar(tolerance["absolute"])
                ),
                "sel_line_match": bool(
                    curve.SpanCount == 1 and is_linear_zero
                ),
                "sel_polyline_match": bool(
                    curve.Degree == 1 and curve.Points.Count > 2
                ),
            }

        return _measure(iterations, curve_classification)

    if kind == "nurbs_curve_extract_points":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        document = Rhino.RhinoDoc.ActiveDoc
        object_id = document.Objects.AddCurve(curve)
        if object_id == System.Guid.Empty:
            raise ValueError("could not add NURBS curve grip probe")
        curve_object = document.Objects.FindId(object_id)
        try:
            curve_object.GripsOn = True
            grips = curve_object.GetGrips()
            if grips is None:
                raise ValueError("could not enable NURBS curve grips")
            return _measure(
                iterations,
                lambda: [_xyz(grip.CurrentLocation) for grip in grips],
            )
        finally:
            curve_object.GripsOn = False
            document.Objects.Delete(object_id, True)

    if kind == "nurbs_curve_divide":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        segment_count = int(operation["segment_count"])
        include_start = bool(operation["include_start"])
        first_index = 0 if include_start else 1
        fractions = System.Array[System.Double](
            [
                float(index) / float(segment_count)
                for index in iteration_range(first_index, segment_count + 1)
            ]
        )
        default_parameters = curve.DivideByCount(segment_count, include_start)
        if default_parameters is None or len(default_parameters) != len(fractions):
            raise ValueError("NURBS curve division returned an unexpected point count")

        def divide_curve():
            parameters = []
            for fraction in fractions:
                # DivideByCount uses RhinoCommon's fixed 1e-8 fractional
                # tolerance. Its point count is checked above; use the public
                # tolerance-bearing solver for coordinate comparisons and
                # leave margin for the external epsilon check.
                success, parameter = curve.NormalizedLengthParameter(
                    fraction, tolerance["relative"] * 0.001
                )
                if not success:
                    raise ValueError("NURBS curve division failed")
                parameters.append(parameter)
            return [_xyz(curve.PointAt(parameter)) for parameter in parameters]

        return _measure(iterations, divide_curve)

    if kind == "nurbs_curve_reverse":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        normalized = _finite(
            operation["normalized_parameter"], "normalized curve parameter"
        )
        if normalized < 0.0 or normalized > 1.0:
            raise ValueError("normalized curve parameter must be in [0, 1]")

        def reverse_curve():
            reversed_curve = curve.DuplicateCurve()
            if reversed_curve is None:
                raise ValueError("could not duplicate NURBS curve")
            try:
                if not reversed_curve.Reverse():
                    raise ValueError("could not reverse NURBS curve")
                parameter = reversed_curve.Domain.ParameterAt(normalized)
                values = reversed_curve.DerivativeAt(parameter, 1)
                if values is None or len(values) < 2:
                    raise ValueError("reversed NURBS curve evaluation failed")
                return {"point": _xyz(values[0]), "derivative": _xyz(values[1])}
            finally:
                reversed_curve.Dispose()

        return _measure(iterations, reverse_curve)

    if kind == "mesh_unify_normals":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])

        def unify_mesh_normals():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                flipped_faces = int(mesh.UnifyNormals())
                if flipped_faces < 0:
                    raise ValueError("mesh face unification failed")
                return {
                    "flipped_faces": flipped_faces,
                    "triangles": _mesh_triangles(mesh),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, unify_mesh_normals)
        finally:
            source.Dispose()

    if kind == "mesh_disjoint_pieces":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])

        def split_disjoint_mesh():
            pieces = source.SplitDisjointPieces()
            if pieces is None:
                raise ValueError("mesh disjoint split failed")
            try:
                return {
                    "disjoint_mesh_count": int(source.DisjointMeshCount),
                    "pieces": [
                        _mesh_value(piece) for piece in pieces
                    ],
                }
            finally:
                for piece in pieces:
                    piece.Dispose()

        try:
            return _measure(iterations, split_disjoint_mesh)
        finally:
            source.Dispose()

    if kind == "mesh_combine_identical_vertices":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        def combine_identical_vertices():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                before = int(mesh.Vertices.Count)
                changed = bool(mesh.Vertices.CombineIdentical(True, True))
                return {
                    "changed": changed,
                    "removed_vertices": before - int(mesh.Vertices.Count),
                    "mesh": _mesh_value(mesh),
                }
            finally:
                mesh.Dispose()
        try:
            return _measure(iterations, combine_identical_vertices)
        finally:
            source.Dispose()

    if kind == "mesh_cull_unused_vertices":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        def cull_unused_vertices():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                removed_vertices = int(mesh.Vertices.CullUnused())
                if removed_vertices < 0:
                    raise ValueError("mesh vertex culling failed")
                return {
                    "changed": removed_vertices > 0,
                    "removed_vertices": removed_vertices,
                    "mesh": _mesh_value(mesh),
                }
            finally:
                mesh.Dispose()
        try:
            return _measure(iterations, cull_unused_vertices)
        finally:
            source.Dispose()

    if kind == "mesh_volume":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        try:
            return _measure(iterations, lambda: float(source.Volume()))
        finally:
            source.Dispose()

    if kind == "mesh_extract_duplicate_faces":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])

        def extract_duplicate_faces():
            remainder = source.DuplicateMesh()
            if remainder is None:
                raise ValueError("could not duplicate mesh")
            extracted = None
            try:
                extracted = remainder.Faces.ExtractDuplicateFaces()
                return {
                    "extracted": (
                        None if extracted is None else _mesh_value(extracted)
                    ),
                    "remainder": _mesh_value(remainder),
                }
            finally:
                if extracted is not None:
                    extracted.Dispose()
                remainder.Dispose()

        try:
            return _measure(iterations, extract_duplicate_faces)
        finally:
            source.Dispose()

    if kind == "mesh_extract_non_manifold":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        selective = operation["selective"]
        if not isinstance(selective, bool):
            source.Dispose()
            raise ValueError("mesh extraction selective flag must be boolean")

        def extract_non_manifold():
            remainder = source.DuplicateMesh()
            if remainder is None:
                raise ValueError("could not duplicate mesh")
            extracted = None
            try:
                extracted = remainder.ExtractNonManifoldEdges(selective)
                return {
                    "extracted": (
                        None if extracted is None else _mesh_value(extracted)
                    ),
                    "remainder": (
                        None if remainder.Faces.Count == 0 else _mesh_value(remainder)
                    ),
                }
            finally:
                if extracted is not None:
                    extracted.Dispose()
                remainder.Dispose()

        try:
            return _measure(iterations, extract_non_manifold)
        finally:
            source.Dispose()

    if kind == "nurbs_surface_extract_points":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        surface = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if surface is None:
            raise ValueError("could not allocate NURBS surface")
        _set_surface_controls(
            surface, operation["control_points"], count_u, count_v
        )
        _set_knots(surface.KnotsU, operation["knots_u"], "surface U knot")
        _set_knots(surface.KnotsV, operation["knots_v"], "surface V knot")
        if not surface.IsValid:
            surface.Dispose()
            raise ValueError("NURBS surface is invalid")
        document = Rhino.RhinoDoc.ActiveDoc
        object_id = document.Objects.AddSurface(surface)
        if object_id == System.Guid.Empty:
            surface.Dispose()
            raise ValueError("could not add NURBS surface grip probe")
        surface_object = document.Objects.FindId(object_id)
        try:
            surface_object.GripsOn = True
            grips = surface_object.GetGrips()
            if grips is None:
                raise ValueError("could not enable NURBS surface grips")
            return _measure(
                iterations,
                lambda: [_xyz(grip.CurrentLocation) for grip in grips],
            )
        finally:
            surface_object.GripsOn = False
            document.Objects.Delete(object_id, True)
            surface.Dispose()

    if kind == "nurbs_surface_evaluate":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        surface = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if surface is None:
            raise ValueError("could not allocate NURBS surface")
        _set_surface_controls(
            surface, operation["control_points"], count_u, count_v
        )
        _set_knots(surface.KnotsU, operation["knots_u"], "surface U knot")
        _set_knots(surface.KnotsV, operation["knots_v"], "surface V knot")
        if not surface.IsValid:
            raise ValueError("NURBS surface is invalid")
        u_value = _finite(operation["u"], "surface U parameter")
        v_value = _finite(operation["v"], "surface V parameter")
        value, elapsed = _measure(
            iterations, lambda: surface.Evaluate(u_value, v_value, 1)
        )
        if value is None or len(value) < 3 or not value[0]:
            raise ValueError("NURBS surface evaluation failed")
        point = value[1]
        derivatives = value[2]
        if derivatives is None or len(derivatives) < 2:
            raise ValueError("NURBS surface derivatives are missing")
        derivative_u = derivatives[0]
        derivative_v = derivatives[1]
        normal = Rhino.Geometry.Vector3d.CrossProduct(derivative_u, derivative_v)
        normal = _unit(normal, tolerance["absolute"], "surface normal")
        return {
            "point": _xyz(point),
            "derivative_u": _xyz(derivative_u),
            "derivative_v": _xyz(derivative_v),
            "normal": _xyz(normal),
        }, elapsed

    raise ValueError("unsupported oracle operation: %s" % kind)


def _validate_request(request):
    if request.get("protocol_version") != PROTOCOL_VERSION:
        raise ValueError(
            "unsupported oracle protocol version %r" % request.get("protocol_version")
        )
    iterations = request.get("iterations", 1)
    if isinstance(iterations, bool) or int(iterations) != iterations:
        raise ValueError("oracle iterations must be an integer")
    iterations = int(iterations)
    if iterations < 1 or iterations > MAX_ITERATIONS:
        raise ValueError("oracle iterations must be from 1 through %d" % MAX_ITERATIONS)
    operations = request.get("operations")
    if not isinstance(operations, list):
        raise ValueError("oracle operations must be an array")
    ids = set()
    for operation in operations:
        operation_id = operation.get("id")
        if not isinstance(operation_id, string_types) or not operation_id.strip():
            raise ValueError("oracle operation id must be a non-empty string")
        if operation_id in ids:
            raise ValueError("duplicated oracle operation id: %s" % operation_id)
        ids.add(operation_id)
    tolerance = dict(DEFAULT_TOLERANCE)
    tolerance.update(request.get("tolerance") or {})
    for name in ("absolute", "relative", "angular"):
        tolerance[name] = _finite(tolerance[name], "%s tolerance" % name)
        if not tolerance[name] > 0.0:
            raise ValueError("%s tolerance must be positive" % name)
    return iterations, operations, tolerance


def _response(request):
    iterations = request.get("iterations", 1) if isinstance(request, dict) else 1
    response = {
        "protocol_version": PROTOCOL_VERSION,
        "engine": "rhino",
        "engine_version": str(Rhino.RhinoApp.Version),
        "iterations": iterations,
        "results": [],
    }
    try:
        iterations, operations, tolerance = _validate_request(request)
        response["iterations"] = iterations
        for operation in operations:
            value, elapsed = _execute(operation, iterations, tolerance)
            response["results"].append(
                {
                    "id": operation["id"],
                    "value": value,
                    "elapsed_ns": elapsed,
                }
            )
    except Exception as error:
        response["results"] = []
        response["error"] = "%s: %s" % (type(error).__name__, error)
    return response


def _main():
    job_directory = os.path.dirname(os.path.abspath(__file__))
    request_path = os.path.join(job_directory, "request.json")
    response_path = os.path.join(job_directory, "response.json")
    temporary_path = response_path + ".tmp"
    request = {}
    try:
        with open(request_path, "r") as stream:
            request = json.load(stream)
        response = _response(request)
    except Exception as error:
        response = {
            "protocol_version": PROTOCOL_VERSION,
            "engine": "rhino",
            "engine_version": str(Rhino.RhinoApp.Version),
            "iterations": 1,
            "results": [],
            "error": "%s: %s" % (type(error).__name__, error),
        }
    with open(temporary_path, "w") as stream:
        json.dump(response, stream, indent=2, allow_nan=False)
        stream.write("\n")
    if os.path.exists(response_path):
        os.remove(response_path)
    os.rename(temporary_path, response_path)
    host_options = {}
    if isinstance(request, dict):
        host_options = request.get("_host") or {}
    if host_options.get("exit_rhino_when_complete"):
        Rhino.RhinoApp.Exit(False)


_main()
