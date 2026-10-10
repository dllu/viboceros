import unittest
from types import SimpleNamespace as NS
from .step_poles_probe import CASES, REGION_CASES, validate, run


class Affine:
    def __init__(self, scale=1., offset=0.):
        self.scale, self.offset = scale, offset
    def __mul__(self, other):
        return Affine(self.scale*other.scale, self.scale*other.offset+self.offset)


class Brep:
    IsValid, IsSolid, SolidOrientation = True, True, 'Outward'
    def __init__(self, x=1.):
        self.x = x
    def DuplicateBrep(self):
        return Brep(self.x)
    def Transform(self, transform):
        self.x = transform.scale*self.x+transform.offset
        return True
    def Dispose(self):
        pass
    def GetType(self):
        return NS(FullName='Brep')


class Reference:
    def __init__(self, transform):
        self.Xform = transform
    def GetType(self):
        return NS(FullName='InstanceReferenceGeometry')


class ObjectTable(list):
    def Delete(self, id_, quiet):
        self[:] = [obj for obj in self if obj.Id != id_]


def instance_host(top):
    table = ObjectTable()
    doc = NS(Objects=table)
    def read(path, doc, options):
        assert options.JoinSurfaces is True and options.LimitFaces is False
        doc.Objects.append(top)
        return True
    rhino = NS(Commands=NS(Command=NS(InCommand=lambda: False)),
        RhinoDoc=NS(ActiveDoc=doc),
        FileIO=NS(FileStpReadOptions=lambda: NS(), FileStp=NS(Read=read)),
        Geometry=NS(Brep=Brep, InstanceReferenceGeometry=Reference,
            Transform=NS(Identity=Affine()),
            VolumeMassProperties=NS(Compute=lambda geometry, *accuracy: NS(Volume=1., Dispose=lambda: None))))
    return dict(Rhino=rhino, _interchange_brep_record=lambda b: dict(x=b.x)), table


def fixture_operation():
    return dict(op='step_regions', id='sphere-cavity', case='sphere-cavity',
        artifact_path='/tmp/viboceros-step-regions-owned/sphere-cavity.step')


class StepPolesProbeTests(unittest.TestCase):
    def test_nested_instance_placement_is_composed_without_mutating_definition_geometry(self):
        source = Brep()
        leaf = NS(Geometry=source)
        child = NS(Geometry=Reference(Affine(3., 4.)),
                   InstanceDefinition=NS(Id='child', GetObjects=lambda: [leaf]))
        top = NS(Id='top', IsInstanceDefinitionGeometry=False,
                 Geometry=Reference(Affine(2., 5.)),
                 InstanceDefinition=NS(Id='top-definition', GetObjects=lambda: [child]))
        host, table = instance_host(top)
        value, _ = run(fixture_operation(), host)
        self.assertEqual(value['top_level_objects'], 1)
        self.assertEqual(value['objects'][0]['instance_depth'], 2)
        self.assertEqual(value['objects'][0]['geometry']['x'], 19.)
        self.assertEqual(source.x, 1.)
        self.assertEqual(table, [])

    def test_cyclic_instances_fail_and_still_clear_owned_import_objects(self):
        top = NS(Id='top', IsInstanceDefinitionGeometry=False, Geometry=Reference(Affine()))
        top.InstanceDefinition = NS(Id='cycle', GetObjects=lambda: [top])
        host, table = instance_host(top)
        with self.assertRaisesRegex(ValueError, 'cyclic'):
            run(fixture_operation(), host)
        self.assertEqual(table, [])
    def test_region_fixture_paths_are_separate_and_bounded(self):
        for case in REGION_CASES:
            operation = dict(op='step_regions', id=case, case=case,
                             artifact_path='/tmp/viboceros-step-regions-owned/' + case + '.step')
            validate(operation)
            operation['artifact_path'] = '/tmp/viboceros-step-poles-owned/' + case + '.step'
            with self.assertRaises(ValueError):
                validate(operation)
    def test_fixed_bounded_artifact_paths(self):
        for case in CASES:
            validate(dict(op='step_poles', id=case, case=case,
                          artifact_path='/tmp/viboceros-step-poles-owned/' + case + '.step'))

    def test_invalid_paths_cases_and_fields(self):
        for changes in [dict(case=[]), dict(case='unknown'), dict(id=''), dict(extra=True),
                        dict(artifact_path='/etc/sphere.step'),
                        dict(artifact_path='/tmp/viboceros-step-poles-owned/sphere.step\n'),
                        dict(artifact_path='/tmp/viboceros-step-poles-owned/sub/sphere.step'),
                        dict(artifact_path='/tmp/viboceros-step-poles-owned/../sphere.step')]:
            operation = dict(op='step_poles', id='sphere', case='sphere',
                             artifact_path='/tmp/viboceros-step-poles-owned/sphere.step')
            operation.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(operation)
