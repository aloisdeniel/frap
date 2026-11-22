# WP 2.11: beveled edges with and without seams and sharp marks: mark_seam and
# mark_sharp carry the marks across the bevel; the rebuilt faces keep seam continuity.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.meshes['Cube'].edges[0].use_seam = True
bpy.data.meshes['Cube'].edges[2].use_edge_sharp = True
bpy.data.meshes['Cube'].edges[5].use_seam = True
bpy.data.meshes['Cube.001'].edges[0].use_seam = True
bpy.data.meshes['Cube.001'].edges[2].use_edge_sharp = True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=3, mark_seam=True, mark_sharp=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Cube.001'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=2)
