# M5+M6 stabilization round 6: `Camera.dof.focus_object` is a reference of the camera data
# (not a user): object.duplicate of the Camera and the Cube together relinks the copied
# camera's focus to Cube.001 (the original keeps the Cube), and removing the focus object
# (`bpy.data.objects.remove`, `object.delete`) clears the reference (it reads None). P_Focus
# holds the bpy.data.objects index of each camera's focus and the Cube's users; the reads
# after the removals are compared by outcome (AttributeError on None).
# golden: startup=factory compare=objects tier=T1 tol=1e-6
import bpy
bpy.data.cameras['Camera'].dof.focus_object = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.duplicate()
bpy.data.objects.new('P_Focus', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Focus'])
bpy.data.objects['P_Focus'].location = (bpy.data.objects.find(bpy.data.objects['Camera'].data.dof.focus_object.name), bpy.data.objects.find(bpy.data.objects['Camera.001'].data.dof.focus_object.name), bpy.data.objects['Cube'].users)
bpy.data.objects['Camera.001'].data.name
bpy.data.objects.remove(bpy.data.objects['Cube.001'])
bpy.data.cameras['Camera.001'].dof.focus_object
bpy.data.cameras['Camera.001'].dof.focus_object.name
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.delete()
bpy.data.cameras['Camera'].dof.focus_object
bpy.data.cameras['Camera'].dof.focus_object.name
