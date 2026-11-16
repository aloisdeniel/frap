import bpy, sys, json, struct, os
out = sys.argv[-1]
os.makedirs(out, exist_ok=True)
def dump(name, **kw):
    p = os.path.join(out, name + ".glb")
    bpy.ops.export_scene.gltf(filepath=p, **kw)
    d = open(p,'rb').read()
    magic, ver, length = struct.unpack_from('<4sII', d, 0)
    jl, jt = struct.unpack_from('<I4s', d, 12)
    js = d[20:20+jl]
    rest = d[20+jl:]
    info = {"magic": magic.decode(), "version": ver, "length": length, "filelen": len(d), "json_len": jl, "json_type": jt.decode(), "json_tail_bytes": list(js[-4:])}
    if rest:
        bl, bt = struct.unpack_from('<I4s', rest, 0)
        info.update(bin_len=bl, bin_type=bt.decode('latin1'))
    j = json.loads(js)
    open(os.path.join(out, name + ".json"), "w").write(json.dumps({"glb": info, "gltf": j}, indent=1))
# 1 default scene (cube, camera, light)
dump("default")
dump("default_all", export_cameras=True, export_lights=True, export_extras=True)
