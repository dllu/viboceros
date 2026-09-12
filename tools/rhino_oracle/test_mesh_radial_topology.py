"""Check the public-topology probe without importing or starting Rhino."""
import ast
import json
from pathlib import Path
from types import SimpleNamespace
import unittest


class MeshRadialTopologyTests(unittest.TestCase):
    def test_recorded_topology_is_consistent_with_source_faces(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/mesh_radial_topology.json").read_text())
        response = json.loads((root / "observations/mesh_radial_topology.json").read_text())
        self.assertEqual(response["engine"], "rhino")
        self.assertNotIn("error", response)
        self.assertEqual(len(response["results"]), 12)
        self.assertEqual(len(request["operations"]), 12)
        for operation, result in zip(request["operations"], response["results"]):
            self.assertEqual(operation["id"], result["id"])
            topology = result["value"]
            raw_to_topology = {}
            for index, vertex in enumerate(topology["vertices"]):
                for raw in vertex["mesh_vertices"]:
                    self.assertNotIn(raw, raw_to_topology)
                    raw_to_topology[raw] = index
                points = {tuple(operation["vertices"][raw]) for raw in vertex["mesh_vertices"]}
                self.assertEqual(len(points), 1)
                self.assertTrue(vertex["sorted"])
                incident = [edge for edge, item in enumerate(topology["edges"]) if index in item["vertices"]]
                self.assertEqual(sorted(vertex["edges_before"]), incident)
                self.assertEqual(sorted(vertex["edges_after"]), incident)
            self.assertEqual(sorted(raw_to_topology), list(range(len(operation["vertices"]))))
            for edge in topology["edges"]:
                expected = [index for index, face in enumerate(operation["triangles"])
                    if set(edge["vertices"]).issubset({raw_to_topology[raw] for raw in face})]
                self.assertEqual(edge["faces"], expected)

    def test_records_raw_membership_incidence_and_sort_order_including_isolated_vertices(self):
        path = Path(__file__).with_name("rhino_worker.py")
        tree = ast.parse(path.read_text())
        definitions = ast.Module(body=[node for node in tree.body
            if isinstance(node, ast.FunctionDef) and node.name == "_mesh_radial_topology_value"], type_ignores=[])
        scope = {}
        exec(compile(definitions, str(path), "exec"), scope)
        edge_order = [[1, 0], [0], [1], None]
        sorted_vertices = []

        def sort_edges(index):
            sorted_vertices.append(index)
            if edge_order[index] is None:
                return False
            edge_order[index].sort()
            return True

        mesh = SimpleNamespace(
            TopologyEdges=SimpleNamespace(
                Count=2,
                GetTopologyVertices=lambda index: SimpleNamespace(I=0, J=index + 1),
                GetConnectedFaces=lambda index: [[7, 3], [5]][index],
            ),
            TopologyVertices=SimpleNamespace(
                Count=4,
                ConnectedEdges=lambda index: edge_order[index],
                SortEdges=sort_edges,
                MeshVertexIndices=lambda index: [[0], [1, 4], [2], [3]][index],
            ),
        )
        result = scope["_mesh_radial_topology_value"](mesh)
        self.assertEqual(sorted_vertices, [0, 1, 2, 3])
        self.assertEqual(result["edges"], [
            {"vertices": [0, 1], "faces": [7, 3]},
            {"vertices": [0, 2], "faces": [5]},
        ])
        self.assertEqual(result["vertices"], [
            {"mesh_vertices": [0], "edges_before": [1, 0], "edges_after": [0, 1], "sorted": True},
            {"mesh_vertices": [1, 4], "edges_before": [0], "edges_after": [0], "sorted": True},
            {"mesh_vertices": [2], "edges_before": [1], "edges_after": [1], "sorted": True},
            {"mesh_vertices": [3], "edges_before": [], "edges_after": [], "sorted": False},
        ])


if __name__ == "__main__":
    unittest.main()
