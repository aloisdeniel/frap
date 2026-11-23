# WP 8.3.6 (T0): object.join_uvs copies the active mesh's active UV map into the other selected
# meshes with as many corners (a target without UV maps gets 'UVMap'; a shared mesh is written
# once); another corner count is skipped with a WARNING; an active mesh without UV maps only
# warns; Edit Mode is left and entered again; the poll needs an active mesh.
# golden: startup=factory compare=objects,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T0
import bpy
bpy.ops.object.join_uvs.poll()
bpy.ops.object.duplicate()
bpy.context.object.location = (3, 0, 0)
bpy.context.object.data.uv_layers.remove(bpy.context.object.data.uv_layers[0])
bpy.ops.object.duplicate()
bpy.context.object.location = (6, 0, 0)
bpy.ops.object.duplicate(linked=True)
bpy.context.object.location = (9, 0, 0)
bpy.ops.mesh.primitive_uv_sphere_add(location=(0, 3, 0))
bpy.context.object.data.uv_layers.new(name='Second')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].data.uv_layers.new(name='Spare')
bpy.data.objects['Cube'].data.uv_layers.active_index = 0
bpy.data.objects['Cube'].data.uv_layers[0].data[0].uv = (0.25, 0.75)
bpy.data.objects['Cube'].data.uv_layers[0].data[5].uv = (0.5, 0.125)
bpy.ops.object.select_all(action='SELECT')
bpy.data.objects['Camera'].select_set(False)
bpy.data.objects['Light'].select_set(False)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.join_uvs()
bpy.data.meshes['Cube.001'].uv_layers.active.name
bpy.data.meshes['Cube.001'].uv_layers.active.data[0].uv
bpy.data.meshes['Cube.002'].uv_layers.active.data[5].uv
bpy.data.objects['Cube'].data.uv_layers.active_index = 1
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.join_uvs()
bpy.context.object.mode
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.data.meshes['Cube.001'].uv_layers.remove(bpy.data.meshes['Cube.001'].uv_layers[0])
bpy.ops.object.join_uvs()
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.join_uvs.poll()
bpy.ops.object.join_uvs()
