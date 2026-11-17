# WP 1.21 (T1), object.shade_* on several selected objects (a mesh shared by two of them is
# shaded once); poll fails with an active Empty; only a camera or nothing selected: CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.shade_smooth()
bpy.data.objects.new('Empty', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Empty'])
bpy.context.view_layer.objects.active = bpy.data.objects['Empty']
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.shade_flat()
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.shade_smooth()
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.shade_smooth()
