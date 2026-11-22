# WP 8.3.13: DATA_TRANSFER refusals: TOPOLOGY vertex mapping with another vertex count (a
# modifier error, the edge data still transfers), a face mapping from a source without
# faces, a non-mesh source (refused by the pointer), no source (disabled).
# golden: startup=empty compare=evaluated:DTopo,evaluated:DNoFace,evaluated:DEmpty,evaluated:DNone tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'S'
bpy.data.objects['S'].vertex_groups.new(name='G')
bpy.data.objects['S'].vertex_groups['G'].add([15, 16, 19, 28, 37], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([11, 32, 34], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([7, 14, 17, 22, 23, 25, 29], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([4, 10, 21, 35], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([8, 41], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([1, 9], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([39], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([0, 26, 27, 30], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([33], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([3, 5, 13, 18, 20, 38], 1.0, 'REPLACE')
bpy.data.objects['S'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes['sharp_edge'].data.foreach_set('value', [True, False, False, False, True, False, False, True, False, False, False, True, False, False, True, False, False, False, False, False, True, False, False, True, True, False, False, False, False, False, False, True, True, False, False, False, False, False, False, True, False, False, False, False, False, False, False, True, True, False, True, False, True, False, True, False, True, False, False, False, False, True, True, True, True, False, True, True, False, False, False])
bpy.ops.object.empty_add()
bpy.context.object.name = 'E'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, size=1)
bpy.context.object.name = 'Wire'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DTopo'
bpy.data.objects['DTopo'].vertex_groups.new(name='G')
bpy.data.objects['DTopo'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DTopo'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DTopo'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DTopo'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DTopo'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DTopo'].modifiers['DT'].vert_mapping = 'TOPOLOGY'
bpy.data.objects['DTopo'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DTopo'].modifiers['DT'].data_types_edges = {'SHARP_EDGE'}
bpy.data.objects['DTopo'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DTopo'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DTopo'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DTopo'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DTopo'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DNoFace'
bpy.data.objects['DNoFace'].vertex_groups.new(name='G')
bpy.data.objects['DNoFace'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNoFace'].modifiers['DT'].object = bpy.data.objects['Wire']
bpy.data.objects['DNoFace'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DNoFace'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DNoFace'].modifiers['DT'].vert_mapping = 'POLY_NEAREST'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DEmpty'
bpy.data.objects['DEmpty'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DEmpty'].modifiers['DT'].object = bpy.data.objects['E']
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DNone'
bpy.data.objects['DNone'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNone'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
