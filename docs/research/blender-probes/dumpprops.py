import bpy, sys, json, addon_utils
print("BLVER", bpy.app.version_string)
for m in addon_utils.modules():
    if m.__name__.endswith("io_scene_gltf2"):
        print("GLTFVER", m.bl_info.get("version"), m.__file__)
def dump(op):
    rna = op.get_rna_type()
    out=[]
    for p in rna.properties:
        if p.identifier=="rna_type": continue
        d={"id":p.identifier,"type":p.type,"name":p.name,"desc":p.description}
        if p.type=="ENUM":
            d["items"]=[(i.identifier,i.name) for i in p.enum_items]
            d["default"]=list(p.default_flag) if p.is_enum_flag else p.default
        elif getattr(p,"array_length",0)>0:
            d["default"]=list(p.default_array)
        elif p.type in ("BOOLEAN","INT","FLOAT","STRING"):
            d["default"]=p.default
        if p.type in ("INT","FLOAT"):
            d["min"]=p.hard_min; d["max"]=p.hard_max
        d["hidden"]=p.is_hidden; d["skip_save"]=p.is_skip_save
        out.append(d)
    return out
res={"export":dump(bpy.ops.export_scene.gltf),"import":dump(bpy.ops.import_scene.gltf)}
open(sys.argv[-1],"w").write(json.dumps(res,indent=1,default=str))
