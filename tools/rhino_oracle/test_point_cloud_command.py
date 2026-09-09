"""Independent analytic checks of PointCloud command measurements."""
import json
from pathlib import Path
import unittest


class PointCloudMeasurements(unittest.TestCase):
    def test_order_duplicates_source_lifetime_and_attributes(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/point_cloud_command.json").read_text())
        response = json.loads((root / "observations/point_cloud_command.json").read_text())
        a, b = [1,2,3], [4,5,6]
        mesh = [[0,0,0],[3,0,0],[0,4,0]]
        # Explicit expected ordered vertices and retained source indices.
        cases = {
            "points-post": ([a,a,b], []),
            "points-pre": ([a,b,a], []),
            "single-point": (None, [0]),
            "mesh": (mesh + [[0,0,0]], [0]),
            "mixed": (mesh + [a], [1,2,3]),
            "point-mesh-pre": ([a] + mesh, [1,2]),
            "single-point-post": (None, [0]),
            "mesh-post": (mesh, [0]),
        }
        self.assertEqual(response["engine"], "rhino")
        self.assertEqual(len(response["results"]), len(cases))
        self.assertEqual({op["id"] for op in request["operations"]}, set(cases))
        for result in response["results"]:
            with self.subTest(case=result["id"]):
                points, retained = cases[result["id"]]
                objects = result["value"]["objects"]
                sources = [o for o in objects if o["original"] is not None]
                outputs = [o for o in objects if o["original"] is None]
                self.assertEqual([o["original"] for o in sources], retained)
                self.assertEqual(len(outputs), int(points is not None))
                if outputs:
                    self.assertEqual(outputs[0], dict(original=None, kind="point_cloud", points=points,
                        layer="Current", name=None, selected=False, color_source="ColorFromLayer", has_colors=False))
                post = next(op.get("postselect", False) for op in request["operations"] if op["id"] == result["id"])
                for source in sources:
                    self.assertEqual(source["layer"], "Source")
                    self.assertEqual(source["name"], "source-%d" % source["original"])
                    self.assertEqual(source["selected"], not post)


if __name__ == "__main__":
    unittest.main()
