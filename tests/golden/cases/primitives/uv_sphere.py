# WP 1.19: mesh.primitive_uv_sphere_add at T1 for vertices; edges, faces, UVs and normals are
# compared canonically (deviation D-19: Blender's edge and face order changes between runs).
# Defaults; segments/ring_count/radius with location; rotation with scale and no UVs;
# align='VIEW'; align='CURSOR' with a moved and rotated 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Sphere,mesh:Sphere.001,mesh:Sphere.002,mesh:Sphere.003,mesh:Sphere.004 tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_uv_sphere_add()
bpy.ops.mesh.primitive_uv_sphere_add(segments=7, ring_count=5, radius=0.5, location=(3, 0, 0))
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=7, radius=2, calc_uvs=False, rotation=(0.3, 0.6, 0.9), scale=(1, 0.5, 2))
bpy.ops.mesh.primitive_uv_sphere_add(segments=5, ring_count=3, align='VIEW')
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.context.scene.cursor.rotation_euler = (-0.4, 0.1, 1.2)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, align='CURSOR')
