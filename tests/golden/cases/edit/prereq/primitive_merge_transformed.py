# WP 2.2: primitives added in Edit Mode go where they would go as objects: the creation matrix
# is relative to the edit object's world matrix (translated, rotated, scaled), with the 3D
# cursor location, an explicit rotation, the operator scale and align='CURSOR'.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (1, 2, 0.5)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0, 0.7)
bpy.data.objects['Cube'].scale = (2, 1, 0.5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cube_add(size=1)
bpy.context.scene.cursor.location = (0, 0, 2)
bpy.ops.mesh.primitive_plane_add(size=1)
bpy.ops.mesh.primitive_cube_add(size=1, location=(-2, 1, 0), rotation=(0.1, 0.2, 0.3), scale=(1, 2, 1))
bpy.context.scene.cursor.rotation_euler = (0, 0.5, 0)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.3, depth=0.5, align='CURSOR', location=(0, 0, -2))
