# WP 8.3.13: object.data_transfer with use_reverse_transfer: the layer item sets swap
# (dt_layers_select_itemf): layers_select_src takes ACTIVE / NAME / INDEX and
# layers_select_dst ACTIVE / ALL / BONE_SELECT / BONE_DEFORM. All groups by name from the
# selected 'Src' to the active 'RAll', the INDEX spelling into 'RIdx', and the forward spelling
# (src='ALL', dst='NAME'), a TypeError with reverse on both sides.
# golden: startup=empty compare=objects,evaluated:RAll,evaluated:RIdx tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'Src'
bpy.data.objects['Src'].vertex_groups.new(name='G')
bpy.data.objects['Src'].vertex_groups['G'].add([9, 30, 34, 41], 0.1, 'REPLACE')
bpy.data.objects['Src'].vertex_groups['G'].add([6, 8, 11, 29, 37], 0.4, 'REPLACE')
bpy.data.objects['Src'].vertex_groups['G'].add([0, 1, 2, 23, 27], 0.9, 'REPLACE')
bpy.data.objects['Src'].vertex_groups.new(name='H')
bpy.data.objects['Src'].vertex_groups['H'].add([3, 4, 5, 12, 13, 20], 0.3, 'REPLACE')
bpy.data.objects['Src'].vertex_groups['H'].add([14, 15, 21, 22, 28], 0.7, 'REPLACE')
bpy.data.objects['Src'].vertex_groups.active_index = 0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'RAll'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Src'].select_set(True)
bpy.data.objects['RAll'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['RAll']
bpy.ops.object.data_transfer(use_reverse_transfer=True, data_type='VGROUP_WEIGHTS', layers_select_src='NAME', layers_select_dst='ALL')
bpy.ops.object.data_transfer(use_reverse_transfer=True, data_type='VGROUP_WEIGHTS', layers_select_src='ALL', layers_select_dst='NAME')
bpy.data.objects['RAll'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['RAll'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['RAll'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['RAll'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['RAll'].modifiers['Dz'].strength = 1.0
bpy.data.objects['RAll'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['RAll'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['RAll'].modifiers['Dx'].direction = 'X'
bpy.data.objects['RAll'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['RAll'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'RIdx'
bpy.data.objects['RIdx'].vertex_groups.new(name='A')
bpy.data.objects['RIdx'].vertex_groups.new(name='B')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Src'].select_set(True)
bpy.data.objects['RIdx'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['RIdx']
bpy.ops.object.data_transfer(use_reverse_transfer=True, data_type='VGROUP_WEIGHTS', layers_select_src='INDEX', layers_select_dst='ALL')
bpy.data.objects['RIdx'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['RIdx'].modifiers['Dz'].vertex_group = 'A'
bpy.data.objects['RIdx'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['RIdx'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['RIdx'].modifiers['Dz'].strength = 1.0
bpy.data.objects['RIdx'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['RIdx'].modifiers['Dx'].vertex_group = 'B'
bpy.data.objects['RIdx'].modifiers['Dx'].direction = 'X'
bpy.data.objects['RIdx'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['RIdx'].modifiers['Dx'].strength = 0.5
