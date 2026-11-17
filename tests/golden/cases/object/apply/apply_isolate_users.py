# WP 1.21 (T1), object.transform_apply(isolate_users=True): the object gets its own copy of the
# shared mesh (Cube.001) before the apply.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube'].scale = (2, 2, 2)
bpy.ops.object.transform_apply(isolate_users=True)
bpy.data.objects['Cube'].data.name
bpy.data.objects['Cube2'].data.name
