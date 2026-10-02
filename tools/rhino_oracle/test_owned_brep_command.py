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


class OwnedBrepFacePickTests(unittest.TestCase):
    def fixture(self, *, available=True, owned=True, success=True, points=True):
        class Faces(list):
            @property
            def Count(self):return len(self)
        face=object();context=Mock();ray=Mock();overlap=Mock()
        references=[Mock(ObjectId='owned' if owned else 'foreign'),Mock(ObjectId='foreign')]
        fixture=OwnedBrepCommand.__new__(OwnedBrepCommand)
        fixture.ids=['owned'];fixture.doc=SimpleNamespace(Objects=Mock(),ModelAbsoluteTolerance=.002)
        fixture.doc.Objects.FindId.return_value=SimpleNamespace(Id='owned',Geometry=SimpleNamespace(Faces=Faces([face])))
        fixture.doc.Objects.PickObjects.return_value=references
        intersection=Mock(return_value=(success,[overlap],['intersection'] if points else []))
        fixture.Rhino=SimpleNamespace(Input=SimpleNamespace(Custom=SimpleNamespace(
            PickContext=lambda:context,PickStyle=SimpleNamespace(PointPick='point'),
            PickMode=SimpleNamespace(Shaded='shaded'))),Geometry=SimpleNamespace(
            LineCurve=Mock(return_value=ray),Intersect=SimpleNamespace(Intersection=SimpleNamespace(CurveBrepFace=intersection))))
        fixture.System=SimpleNamespace(Drawing=SimpleNamespace(Rectangle=lambda *args:args))
        viewport=Mock();viewport.GetFrustumLine.return_value=(available,'line')
        return fixture,viewport,context,references,ray,overlap,intersection,face

    def test_owned_object_and_trimmed_face_must_both_hit_without_selecting(self):
        fixture,viewport,context,references,ray,overlap,intersection,face=self.fixture()
        fixture.verify_face_pick(0,0,'view',viewport,40,60)
        intersection.assert_called_once_with(ray,face,.002)
        self.assertEqual(context.PickMode,'shaded')
        self.assertEqual([call[0] for call in fixture.doc.Objects.method_calls],['FindId','PickObjects'])
        for resource in [context,ray,overlap]+references:resource.Dispose.assert_called_once()

    def test_missing_ray_foreign_object_and_no_face_intersection_are_rejected(self):
        for failure in ('available','owned','success','points'):
            with self.subTest(failure=failure):
                fixture,viewport,context,references,ray,overlap,intersection,face=self.fixture(**{failure:False})
                with self.assertRaises(ValueError):fixture.verify_face_pick(0,0,'view',viewport,40,60)
                context.Dispose.assert_called_once()
                if failure!='available':
                    for reference in references:reference.Dispose.assert_called_once()
                if failure in ('success','points'):
                    ray.Dispose.assert_called_once();overlap.Dispose.assert_called_once()

    def test_missing_source_and_invalid_indices_fail_before_allocating_context(self):
        for index in (-1,1,True):
            fixture,viewport,context,*rest=self.fixture()
            with self.subTest(index=index),self.assertRaises(ValueError):fixture.verify_face_pick(0,index,'view',viewport,40,60)
            viewport.GetFrustumLine.assert_not_called();context.Dispose.assert_not_called()
        fixture,viewport,context,*rest=self.fixture();fixture.doc.Objects.FindId.return_value=None
        with self.assertRaises(ValueError):fixture.verify_face_pick(0,0,'view',viewport,40,60)
        viewport.GetFrustumLine.assert_not_called()

    def test_intersection_exception_releases_the_ray_and_all_references(self):
        fixture,viewport,context,references,ray,overlap,intersection,face=self.fixture()
        intersection.side_effect=RuntimeError('intersection failed')
        with self.assertRaisesRegex(RuntimeError,'intersection failed'):fixture.verify_face_pick(0,0,'view',viewport,40,60)
        for resource in [context,ray]+references:resource.Dispose.assert_called_once()

    def test_one_disposal_failure_does_not_leak_remaining_references(self):
        fixture,viewport,context,references,ray,overlap,intersection,face=self.fixture()
        overlap.Dispose.side_effect=RuntimeError('dispose failed')
        with self.assertRaisesRegex(ValueError,'cleanup failed'):fixture.verify_face_pick(0,0,'view',viewport,40,60)
        for resource in [context,ray,overlap]+references:resource.Dispose.assert_called_once()
