# WP 3.9: a toy robot built as a hierarchy. A root empty with a quaternion rotation carries a
# torso cube, which carries a head (axis-angle rotation), two arms (ZXY Euler, one mirrored with
# a negative X scale, which flips its winding) and an antenna empty with a cone child. Parents
# are set through the operator (object.parent_set keeps the world transform with a parent
# inverse), node TRS are composed by the exporter. Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/robot.glb,glb:out/robot_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Camera'])
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.data.objects['Cube'].name = 'Torso'
bpy.data.objects['Torso'].location = (0, 0, 2)
bpy.data.objects['Torso'].scale = (0.8, 0.5, 1.0)
bpy.ops.object.empty_add(type='PLAIN_AXES', location=(0, 0, 0))
bpy.data.objects['Empty'].name = 'Root'
bpy.data.objects['Root'].rotation_mode = 'QUATERNION'
bpy.data.objects['Root'].rotation_quaternion = (0.9238795, 0, 0, 0.3826834)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=0.5, location=(0, 0, 3.5))
bpy.data.objects['Sphere'].name = 'Head'
bpy.data.objects['Head'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Head'].rotation_axis_angle = (0.4, 0, 0, 1)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.2, depth=1.5, location=(1.2, 0, 2))
bpy.data.objects['Cylinder'].name = 'ArmL'
bpy.data.objects['ArmL'].rotation_mode = 'ZXY'
bpy.data.objects['ArmL'].rotation_euler = (0.3, 0.2, 0.1)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.2, depth=1.5, location=(-1.2, 0, 2))
bpy.data.objects['Cylinder'].name = 'ArmR'
bpy.data.objects['ArmR'].scale = (-1, 1, 1)
bpy.ops.object.empty_add(type='SINGLE_ARROW', location=(0, 0, 4))
bpy.data.objects['Empty'].name = 'Antenna'
bpy.ops.mesh.primitive_cone_add(vertices=6, radius1=0.1, depth=0.4, location=(0, 0, 4.3))
bpy.data.objects['Cone'].name = 'Tip'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Tip'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Antenna']
bpy.ops.object.parent_set(type='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Antenna'].select_set(True)
bpy.data.objects['Head'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Head']
bpy.ops.object.parent_set(type='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Head'].select_set(True)
bpy.data.objects['ArmL'].select_set(True)
bpy.data.objects['ArmR'].select_set(True)
bpy.data.objects['Torso'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Torso']
bpy.ops.object.parent_set(type='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Torso'].select_set(True)
bpy.data.objects['Root'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Root']
bpy.ops.object.parent_set(type='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/robot.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/robot.glb')
bpy.ops.export_scene.gltf(filepath='out/robot_rt.glb')
