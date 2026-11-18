# WP 2.13 workflow (mug): a cylinder with its top cap deleted (an open cup), given wall
# thickness with mesh.solidify (side quads in canonical order, D-65), then shaded smooth.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=20, radius=0.5, depth=1, location=(0, 0, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.solidify(thickness=0.05)
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
