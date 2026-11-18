# WP 2.13 workflow (mug): a body from a cylinder, then a handle: a small circle added in Edit
# Mode against the side (only it is selected), filled, and spun half a turn around a
# horizontal axis beside the body.
# golden: startup=empty compare=objects,context,mesh:Mug tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.5, depth=1, location=(0, 0, 0.5))
bpy.context.object.name = 'Mug'
bpy.context.object.data.name = 'Mug'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=0.06, location=(0.48, 0, 0.8))
bpy.data.meshes['Mug'].total_vert_sel
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.spin(steps=8, angle=-3.141592653589793, center=(0.48, 0, 0.5), axis=(0, 1, 0))
bpy.data.meshes['Mug'].total_face_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
