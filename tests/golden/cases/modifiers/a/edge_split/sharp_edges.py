# WP 4.2: EDGE_SPLIT by sharp edges only (`use_edge_angle` off): the four edges of a face and
# one more, marked sharp in Edit Mode, split, nothing else; with both options off the
# modifier passes the mesh through.
# golden: startup=empty compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 5])
bpy.ops.mesh.mark_sharp()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='EdgeSplit', type='EDGE_SPLIT')
bpy.data.objects['Cube'].modifiers['EdgeSplit'].use_edge_angle = False
bpy.data.objects['Cube.001'].modifiers.new(name='EdgeSplit', type='EDGE_SPLIT')
bpy.data.objects['Cube.001'].modifiers['EdgeSplit'].use_edge_angle = False
bpy.data.objects['Cube.001'].modifiers['EdgeSplit'].use_edge_sharp = False
