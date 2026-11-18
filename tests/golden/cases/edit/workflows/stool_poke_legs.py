# WP 2.13 workflow (stool): a round seat from a thin cylinder; its bottom cap (picked by its
# normal) is poked into a fan of triangles, three triangles 120 degrees apart are inset
# individually and extruded down as legs, splayed out by scaling about the seat's centre.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.35, depth=0.06, location=(0, 0, 0.6))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, -1))
bpy.ops.mesh.poke()
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.frap.select_elements(type='FACE', indices=[14, 18, 22])
bpy.ops.mesh.inset(thickness=0.03, use_individual=True)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.55)})
bpy.ops.transform.resize(value=(1.6, 1.6, 1))
bpy.data.meshes['Cylinder'].total_face_sel
