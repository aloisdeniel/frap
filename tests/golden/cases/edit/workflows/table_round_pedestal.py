# WP 2.13 workflow (table): a round top from a thin cylinder; the bottom cap, picked by its
# normal, is inset to a small disc, extruded down as the pedestal, extruded again and scaled
# out as the foot, then the whole table is shaded smooth.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=1, depth=0.08, location=(0, 0, 0.75))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, -1))
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.mesh.inset(thickness=0.85)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.65)})
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.04)})
bpy.ops.transform.resize(value=(3.5, 3.5, 1))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
