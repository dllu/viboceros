"""Reject mistargeted native mouse inputs before observing command no-ops."""
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

from .owned_brep_command import OwnedBrepCommand


class OwnedBrepPickTests(unittest.TestCase):
    def fixture(self, *, ray=True, owned=True, hit=True):
        curve=Mock();context=Mock();context.PickFrustumTest.return_value=(hit,0.,0.,0.)
        reference=Mock(ObjectId='owned' if owned else 'foreign')
        obj=SimpleNamespace(Id='owned',Geometry=SimpleNamespace(
            Edges=[SimpleNamespace(ToNurbsCurve=Mock(return_value=curve))]))
        fixture=OwnedBrepCommand.__new__(OwnedBrepCommand)
        fixture.ids=['owned'];fixture.doc=SimpleNamespace(Objects=Mock())
        fixture.doc.Objects.FindId.return_value=obj
        fixture.doc.Objects.PickObjects.return_value=[reference]
        fixture.Rhino=SimpleNamespace(Input=SimpleNamespace(Custom=SimpleNamespace(
            PickContext=lambda:context,PickStyle=SimpleNamespace(PointPick='point'))))
        fixture.System=SimpleNamespace(Drawing=SimpleNamespace(Rectangle=lambda *args:args))
        viewport=Mock();viewport.GetFrustumLine.return_value=(ray,'ray')
        return fixture,viewport,context,reference,curve

    def test_owned_object_and_intended_edge_must_both_hit(self):
        fixture,viewport,context,reference,curve=self.fixture()
        fixture.verify_edge_pick(0,0,'view',viewport,40,60)
        context.PickFrustumTest.assert_called_once_with(curve)
        curve.Dispose.assert_called_once();reference.Dispose.assert_called_once()
        context.Dispose.assert_called_once()
        fixture.doc.Objects.FindId.assert_called_once_with('owned')

    def test_unavailable_ray_wrong_object_and_missed_edge_are_rejected(self):
        for failure in ('ray','owned','hit'):
            with self.subTest(failure=failure):
                fixture,viewport,context,reference,curve=self.fixture(**{failure:False})
                with self.assertRaises(ValueError):fixture.verify_edge_pick(0,0,'view',viewport,40,60)
                context.Dispose.assert_called_once()
                if failure!='ray':reference.Dispose.assert_called_once()
                if failure=='hit':curve.Dispose.assert_called_once()
