# WP 2.11: the four top edges of a box (a loop: two beveled edges and one unbeveled
# edge at each corner, loop slide and width adjustment), then without loop slide.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T2 order=exact
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), scale=(1, 2, 1))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.bevel(offset=0.3, segments=3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Cube.001'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.bevel(offset=0.3, segments=2, loop_slide=False)
