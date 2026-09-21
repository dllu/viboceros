# -*- coding: utf-8 -*-
"""Public viewport calibration for independently replaying bounded owned clicks."""
import math


def capture(viewport, point, pixel, host):
    systems = host["Rhino"].DocObjects.CoordinateSystem
    transform = viewport.GetTransform(systems.World, systems.Screen)
    matrix = [[float(transform[i,j]) for j in range(4)] for i in range(4)]
    if any(math.isnan(v) or math.isinf(v) for row in matrix for v in row):
        raise ValueError("nonfinite viewport transform")
    world = host["_xyz"](point)
    camera = host["_xyz"](viewport.CameraLocation)
    direction = host["_xyz"](viewport.CameraDirection)
    if any(math.isnan(v) or math.isinf(v) for v in world + camera + direction):
        raise ValueError("nonfinite viewport pose or aim")
    if not any(direction): raise ValueError("zero viewport camera direction")
    homogeneous = [sum(row[j] * (world + [1.])[j] for j in range(4)) for row in matrix]
    if homogeneous[3] == 0: raise ValueError("viewport aim lies on projection plane")
    projected = [homogeneous[i] / homogeneous[3] for i in (0,1)]
    client = viewport.WorldToClient(point)
    client_xy = [float(client.X),float(client.Y)]
    if any(math.isnan(v) or math.isinf(v) for v in projected + client_xy) or max(abs(projected[0]-client.X),abs(projected[1]-client.Y)) > 1e-7:
        raise ValueError("viewport matrix disagrees with WorldToClient")
    return dict(world_to_screen=matrix, aim=world, aim_client=client_xy,
                click_client=pixel, size=[int(viewport.Size.Width),int(viewport.Size.Height)],
                camera_location=camera, camera_direction=direction,
                perspective=bool(viewport.IsPerspectiveProjection))
