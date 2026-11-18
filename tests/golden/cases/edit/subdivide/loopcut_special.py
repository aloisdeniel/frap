# WP 2.6: mesh.loopcut special cases: an edge with no adjacent quad (a cone side) only splits
# the edge and deselects its end vertices; a wire edge (a circle without fill) selects its new
# vertices and edges; a ring stopped by n-gons; a second edit object picked by object_index.
# golden: startup=factory compare=objects,context,mesh:Cone,mesh:Circle,mesh:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=6, location=(-3, 0, 0))
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.loopcut(number_cuts=2, edge_index=0, object_index=0)
bpy.ops.mesh.loopcut(number_cuts=3, edge_index=1, object_index=1)
bpy.ops.mesh.loopcut(number_cuts=1, edge_index=4, object_index=2)
bpy.ops.mesh.loopcut(number_cuts=1, edge_index=0, object_index=3)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.subdivide()
bpy.ops.mesh.loopcut(number_cuts=2, edge_index=0, object_index=0)
bpy.ops.mesh.loopcut(number_cuts=1, edge_index=13, object_index=0)
