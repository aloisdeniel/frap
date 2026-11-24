# M8 stabilization round 7: Blender reads and writes some collection properties through its
# raw-array fast path (mesh attribute data, `edges` 'vertices', `polygons` 'loop_start',
# `loop_triangles` 'loops', the normal collections, `uv_layers[i].uv`, shape key `points`):
# there a `foreach_get` / `foreach_set` buffer must hold exactly the values (a longer one
# raises and writes nothing), while the item-by-item path (`vertices` 'co', `polygons`
# 'vertices', `uv_layers[i].data`, shape key `data`) takes a longer one (round 6 accepted a
# longer buffer everywhere). A buffer for an empty collection raises TypeError; `foreach_get`
# needs a list (a tuple raises TypeError once the size fits); `foreach_set` converts every
# item first (a float for an int property, 2 for a boolean, a nested list raise TypeError).
# Blender writes the buffer before it raises that TypeError, a bad item as -1 (D-173): those
# lines run on a second cube that is removed before the dump (the out-of-range
# `material_index` one too: Blender adds the attribute before it raises).
# golden: startup=factory tier=T0 compare=mesh:Cube
import bpy
bpy.data.meshes['Cube'].attributes['position'].data.foreach_set('vector', [0.5] * 25)
bpy.data.meshes['Cube'].attributes['position'].data.foreach_get('vector', [0.0] * 25)
bpy.data.meshes['Cube'].attributes['position'].data.foreach_get('vector', [0.0] * 24)
bpy.data.meshes['Cube'].vertices.foreach_set('co', [-1, -1, -1, -1, -1, 1, -1, 1, -1, -1, 1, 1, 1, -1, -1, 1, -1, 1, 1, 1, -1, 1, 1, 2, 5, 5, 5])
bpy.data.meshes['Cube'].vertices.foreach_get('co', [0.0] * 25)
bpy.data.meshes['Cube'].attributes.new('fv', 'FLOAT', 'FACE')
bpy.data.meshes['Cube'].attributes['fv'].data.foreach_set('value', [1.5] * 7)
bpy.data.meshes['Cube'].attributes['fv'].data.foreach_set('value', [1.5] * 6)
bpy.data.meshes['Cube'].color_attributes.new('F', 'FLOAT_COLOR', 'CORNER')
bpy.data.meshes['Cube'].color_attributes['F'].data.foreach_set('color', [0.5] * 97)
bpy.data.meshes['Cube'].color_attributes['F'].data.foreach_get('color', [0.0] * 97)
bpy.data.meshes['Cube'].color_attributes['F'].data.foreach_get('color_srgb', [0.0] * 97)
bpy.data.meshes['Cube'].polygons.foreach_set('loop_start', [0] * 7)
bpy.data.meshes['Cube'].polygons.foreach_get('loop_start', [0] * 7)
bpy.data.meshes['Cube'].polygons.foreach_set('loop_start', [0, 4, 8, 12, 16, 20])
bpy.data.meshes['Cube'].polygons.foreach_get('loop_total', [0] * 7)
bpy.data.meshes['Cube'].edges.foreach_set('vertices', [0] * 25)
bpy.data.meshes['Cube'].edges.foreach_get('vertices', [0] * 25)
bpy.data.meshes['Cube'].edges.foreach_get('vertices', [0] * 24)
bpy.data.meshes['Cube'].edges.foreach_set('select', [True] * 13)
bpy.data.meshes['Cube'].calc_loop_triangles()
bpy.data.meshes['Cube'].loop_triangles.foreach_get('loops', [0] * 37)
bpy.data.meshes['Cube'].loop_triangles.foreach_get('vertices', [0] * 37)
bpy.data.meshes['Cube'].loop_triangle_polygons.foreach_get('value', [0] * 13)
bpy.data.meshes['Cube'].polygon_normals.foreach_get('vector', [0.0] * 19)
bpy.data.meshes['Cube'].vertex_normals.foreach_get('vector', [0.0] * 25)
bpy.data.meshes['Cube'].corner_normals.foreach_get('vector', [0.0] * 73)
bpy.data.meshes['Cube'].polygons.foreach_get('normal', [0.0] * 19)
bpy.data.meshes['Cube'].uv_layers[0].uv.foreach_get('vector', [0.0] * 49)
bpy.data.meshes['Cube'].uv_layers[0].uv.foreach_set('vector', [0.5] * 49)
bpy.data.meshes['Cube'].uv_layers[0].data.foreach_get('uv', [0.0] * 49)
bpy.data.meshes['Cube'].polygons.foreach_get('vertices', (0,) * 24)
bpy.data.meshes['Cube'].polygons.foreach_get('vertices', (0,) * 3)
bpy.data.meshes['Cube'].edges.foreach_get('vertices', (0,) * 25)
bpy.ops.mesh.primitive_cube_add()
bpy.data.meshes['Cube.001'].polygons.foreach_set('vertices', [4, 6, 2, 0, 3, 2, 6, 7, 7, 6, 4, 5, 5, 1, 3, 7, 1, 0, 2, 3, 5, 4, 0, 1.5])
bpy.data.meshes['Cube.001'].vertices.foreach_set('co', [[1, 1, 1]] * 8)
bpy.data.meshes['Cube.001'].vertices.foreach_set('co', [1.0] * 23 + ['a'])
bpy.data.meshes['Cube.001'].edges.foreach_set('vertices', [1.5] + [1] * 23)
bpy.data.meshes['Cube.001'].polygons.foreach_set('material_index', [2 ** 40] * 6)
bpy.data.objects.remove(bpy.data.objects['Cube.001'])
bpy.data.meshes.remove(bpy.data.meshes['Cube.001'])
bpy.data.meshes['Cube'].vertices.foreach_set('select', [2] * 8)
bpy.data.meshes['Cube'].vertices.foreach_set('select', [1.0] * 8)
bpy.data.meshes['Cube'].vertices.foreach_set('index', [0.5] * 30)
bpy.data.meshes['Cube'].vertices.foreach_set('select', [0, 1] * 4)
bpy.data.meshes.new('E')
bpy.data.meshes['E'].vertices.foreach_get('co', [0.0] * 3)
bpy.data.meshes['E'].edges.foreach_set('foo', [0])
bpy.data.meshes['E'].vertices.foreach_get('co', [])
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.shape_keys['Key'].key_blocks['Basis'].points.foreach_get('co', [0.0] * 25)
bpy.data.shape_keys['Key'].key_blocks['Basis'].points.foreach_set('co', [0.0] * 25)
bpy.data.shape_keys['Key'].key_blocks['Basis'].data.foreach_get('co', [0.0] * 25)
