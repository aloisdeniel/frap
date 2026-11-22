# WP 8.3.4: object.add for every type frap has data for (MESH, CAMERA, LIGHT, ARMATURE, EMPTY,
# POINTCLOUD): the object and its data are named after the type and uniquified independently;
# placement from the cursor, location, rotation and align (WORLD, VIEW, CURSOR); scale and
# radius never scale the object; enter_editmode enters Edit Mode for a mesh or an armature and is
# ignored by a camera, a light and an empty; the data exist (read back by name); poll Object Mode.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.context.scene.cursor.rotation_euler = (0.1, 0.2, 0.3)
bpy.ops.object.add()
bpy.ops.object.add(type='MESH', radius=3, location=(1, 0, 0), rotation=(0.1, 0.2, 0.3), scale=(2, 2, 2))
bpy.data.meshes['Mesh']
bpy.ops.object.add(type='CAMERA', align='VIEW', radius=2)
bpy.data.cameras['Camera.001']
bpy.ops.object.add(type='LIGHT', align='CURSOR', radius=4)
bpy.data.lights['Light.001']
bpy.ops.object.add(type='ARMATURE', location=(0, 0, 5))
bpy.data.armatures['Armature']
bpy.ops.object.add(type='POINTCLOUD', location=(0, 5, 0))
bpy.data.pointclouds['PointCloud']
bpy.ops.object.add(type='EMPTY', radius=0.5, location=(5, 0, 0))
bpy.ops.object.add(type='CAMERA', enter_editmode=True)
bpy.ops.object.add(type='EMPTY', enter_editmode=True)
bpy.ops.object.add(type='ARMATURE', enter_editmode=True)
bpy.ops.object.add(type='MESH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.add(type='MESH', enter_editmode=True, location=(-1, -1, -1))
bpy.ops.object.add(type='LIGHT')
bpy.ops.object.add(type='MESHH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Mesh.001'].name = 'EditedMesh'
bpy.ops.object.add(type='MESH', enter_editmode=True)
