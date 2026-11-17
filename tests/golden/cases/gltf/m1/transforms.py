# WP 1.23: object transforms in several rotation modes, non-uniform and negative scale, delta
# transforms; Y-up conversion of the node TRS.
# golden: startup=factory tier=T2 compare=glb:out/transforms.glb tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (1, -2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.3, -0.7, 1.2)
bpy.data.objects['Cube'].scale = (1, 2, 0.5)
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(-3, 0, 0))
bpy.data.objects['Cone'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cone'].rotation_quaternion = (0.8, 0.2, 0.4, -0.4)
bpy.data.objects['Cone'].scale = (-1, 1, 1)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(3, 3, 0))
bpy.data.objects['Cylinder'].rotation_mode = 'ZXY'
bpy.data.objects['Cylinder'].rotation_euler = (1.0, 0.5, -0.25)
bpy.data.objects['Cylinder'].delta_location = (0, 0, 2)
bpy.data.objects['Cylinder'].delta_scale = (2, 2, 2)
bpy.ops.export_scene.gltf(filepath='out/transforms.glb')
