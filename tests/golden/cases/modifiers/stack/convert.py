# WP 4.1 (T1): object.convert(target='MESH') writes the evaluated mesh into the data and clears
# the modifiers; a mesh shared by two selected objects is converted once (the other only
# loses its modifiers); keep_original=True converts a selected, active copy; cameras are
# incompatible (INFO), an empty has no evaluated mesh (WARNING); nothing selected is
# CANCELLED; a selected object in Edit Mode raises; the poll needs Object Mode.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder,mesh:Cylinder.001 tier=T1
import bpy
bpy.ops.object.shade_auto_smooth(angle=0.3)
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].modifiers.new('W', 'WELD')
bpy.data.objects['Cube2'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.convert(target='MESH')
bpy.ops.mesh.primitive_cylinder_add(vertices=10, location=(3, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=0.5)
bpy.ops.object.convert(target='MESH', keep_original=True)
bpy.data.objects.new('Empty', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Empty'])
bpy.data.objects['Empty'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.convert(target='MESH')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.convert(target='MESH')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.convert(target='MESH')
bpy.data.objects['Cylinder'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cylinder']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.convert(target='MESH')
bpy.ops.object.convert.poll()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Cylinder'].select_set(False)
