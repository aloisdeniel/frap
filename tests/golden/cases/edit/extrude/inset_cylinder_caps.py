# WP 2.5: mesh.inset on the n-gon caps of a cylinder: individually, then as a region with the
# ring selected.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=7)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[7, 8])
bpy.ops.mesh.inset(thickness=0.2, depth=0.1, use_individual=True)
bpy.ops.mesh.inset(thickness=0.1, use_select_inset=True)
bpy.data.meshes['Cylinder'].total_face_sel
