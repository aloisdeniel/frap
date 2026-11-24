# M8 stabilization round 8: `vertices.add(count)`, `edges.add`, `loops.add` and
# `polygons.add` append default elements (round 7 raised NotImplementedError), so a mesh can
# be built with `foreach_set` and `update(calc_edges=True)`. New vertices sit at the origin
# with the CustomData defaults, new edges are (0, 0), new corners use vertex 0, new faces
# start where the last face ended. A new FLOAT4X4 attribute holds zero matrices (round 7:
# identity).
# golden: startup=factory tier=T0 compare=mesh:Q,mesh:Cube
import bpy
bpy.data.meshes.new('Q')
bpy.data.meshes['Q'].vertices.add(4)
bpy.data.meshes['Q'].vertices.foreach_set('co', [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0])
bpy.data.meshes['Q'].loops.add(4)
bpy.data.meshes['Q'].loops.foreach_set('vertex_index', [0, 1, 2, 3])
bpy.data.meshes['Q'].polygons.add(1)
bpy.data.meshes['Q'].polygons.foreach_set('loop_start', [0])
bpy.data.meshes['Q'].update(calc_edges=True)
bpy.data.meshes['Q'].attributes.new('m', 'FLOAT4X4', 'FACE')
bpy.data.meshes['Q'].attributes.new('q', 'QUATERNION', 'POINT')
bpy.data.meshes['Q'].attributes.new('c', 'FLOAT_COLOR', 'CORNER')
bpy.data.meshes['Q'].vertices.add(1)
bpy.data.meshes['Q'].edges.add(1)
bpy.data.meshes['Q'].edges.foreach_get('vertices', [0] * 10)
bpy.data.meshes['Q'].vertices.add(0)
bpy.data.meshes['Q'].vertices.add(-1)
bpy.data.meshes['Q'].vertices.add(1.5)
bpy.data.meshes['Q'].vertices.add()
bpy.data.meshes['Q'].vertices.add(count=1)
bpy.data.meshes.new('R')
bpy.data.meshes['R'].loops.add(4)
bpy.data.meshes['R'].polygons.add(2)
bpy.data.meshes['R'].polygons.foreach_get('loop_start', [0] * 2)
bpy.data.meshes['R'].polygons.foreach_get('loop_total', [0] * 2)
bpy.data.meshes['R'].edges.add(2)
bpy.data.meshes['R'].edges.foreach_get('vertices', [0] * 4)
bpy.data.meshes['Cube'].attributes.new('m', 'FLOAT4X4', 'POINT')
bpy.data.meshes['Cube'].attributes.new('q', 'QUATERNION', 'EDGE')
bpy.data.meshes['Cube'].attributes.new('p', 'QUATERNION', 'POINT')
bpy.data.meshes['Cube'].attributes.new('c', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Cube'].attributes['m'].data.foreach_get('value', [0.0] * 128)
bpy.data.meshes['Cube'].vertices.add(2)
bpy.data.meshes['Cube'].edges.add(1)
bpy.data.meshes['Cube'].vertices.foreach_get('co', [0.0] * 30)
