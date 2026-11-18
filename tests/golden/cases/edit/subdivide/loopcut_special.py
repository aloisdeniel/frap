# WP 2.6: mesh.loopcut special cases, objects indexed active first (Circle), then in base order
# (Cube, Cone): a wire edge (circle without fill) selects its new vertices and edges; a cube
# ring; an edge with no adjacent quad (cone) only splits the edge and deselects its ends; an
# out-of-range object_index cancels; then rings stopped by n-gons on a subdivided cube.
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
