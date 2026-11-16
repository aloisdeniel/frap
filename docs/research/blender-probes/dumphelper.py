def dump(name, **kw):
    import json, struct, os
    p = os.path.join(out, name + ".glb")
    bpy.ops.export_scene.gltf(filepath=p, **kw)
    d = open(p,'rb').read()
    magic, ver, length = struct.unpack_from('<4sII', d, 0)
    jl, jt = struct.unpack_from('<I4s', d, 12)
    js = d[20:20+jl]
    j = json.loads(js)
    open(os.path.join(out, name + ".json"), "w").write(json.dumps(j, indent=1))
