# WP 1.22: `bpy.context.evaluated_depsgraph_get()`, `ID.evaluated_get(depsgraph)`,
# `Object.to_mesh()`, `Object.to_mesh_clear()` (a no-op on frap's side, never clearing the
# original data), `ID.original` (the evaluated dump of `compare=evaluated:` uses the same path) and
# `view_layer.update()`; objects without geometry and bad depsgraph arguments raise Blender's
# errors.
# golden: startup=factory tier=T0 compare=objects,evaluated:Cube,evaluated:Tri
import bpy
bpy.context.evaluated_depsgraph_get()
bpy.context.evaluated_depsgraph_get(1)
bpy.context.view_layer.update()
bpy.context.view_layer.update(1)
bpy.data.objects['Cube'].location = (0, 0, 1)
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).name
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh().name
bpy.data.objects['Cube'].to_mesh().name
bpy.data.objects['Cube'].to_mesh(preserve_all_data_layers=True, depsgraph=bpy.context.evaluated_depsgraph_get()).name
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).matrix_world
bpy.data.meshes['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).name
bpy.data.materials['Material'].evaluated_get(bpy.context.evaluated_depsgraph_get()).name
bpy.data.objects['Cube'].evaluated_get(None)
bpy.data.objects['Cube'].evaluated_get()
bpy.data.objects['Cube'].evaluated_get(bpy.data.objects['Cube'])
bpy.data.objects['Camera'].to_mesh()
bpy.data.objects['Light'].evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh()
bpy.data.objects.new('Empty', None)
bpy.data.objects['Empty'].to_mesh()
bpy.data.objects.new('Tri', bpy.data.meshes.new('Tri'))
bpy.data.meshes['Tri'].from_pydata([(0, 0, 0), (1, 0, 0), (0, 1, 0)], [], [(0, 1, 2)])
bpy.context.scene.collection.objects.link(bpy.data.objects['Tri'])
bpy.data.objects['Tri'].evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh().name
bpy.data.objects['Cube'].to_mesh_clear()
bpy.data.objects['Cube'].to_mesh_clear()
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh_clear()
bpy.data.objects['Camera'].to_mesh_clear()
bpy.data.objects['Cube'].to_mesh_clear(1)
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).original.name
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).original.location.x = 2
bpy.data.meshes['Cube'].vertices[0].co.x
