"""Native SDK partitions of private duplicates, independent of region selection."""


def capture(case, ids, host):
    from mesh_split_sdk import capture as split
    doc = host['Rhino'].RhinoDoc.ActiveDoc
    geometry = [doc.Objects.FindId(identifier).Geometry for identifier in ids]
    before = [int(g.DataCRC(0)) for g in geometry]
    rows = split(geometry, dict(first=[2], second=case['sdk_cutters'],
                 split_coplanar=True, create_ngons=False, sdk_tolerance=doc.ModelAbsoluteTolerance), host)
    if before != [int(g.DataCRC(0)) for g in geometry]:
        raise ValueError('private SDK split modified live owned geometry')
    return rows
