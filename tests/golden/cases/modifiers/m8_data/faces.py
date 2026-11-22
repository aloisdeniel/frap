# WP 8.3.13: DATA_TRANSFER of the face data (flat/smooth, freestyle mark) with every
# poly_mapping: TOPOLOGY, NEAREST, NORMAL (with a ray radius too) and the seeded random
# rays of POLYINTERP_PNORPROJ (the most-weighted source face wins).
# golden: startup=empty compare=evaluated:DTopo,evaluated:DNearest,evaluated:DNormal,evaluated:DNormalRay,evaluated:DPnorproj,evaluated:DPnorprojRay tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'S'
bpy.ops.object.shade_flat()
bpy.data.objects['S'].data.attributes['sharp_face'].data.foreach_set('value', [True, False, False, False, False, True, False, False, False, True, False, True, True, True, True, True, False, False, True, False, False, True, True, True, True, True, False, True, False, True])
bpy.data.objects['S'].data.attributes.new(name='freestyle_face', type='BOOLEAN', domain='FACE')
bpy.data.objects['S'].data.attributes['freestyle_face'].data.foreach_set('value', [False, True, False, False, True, False, False, True, False, False, False, False, False, False, True, False, True, True, False, False, True, True, False, False, False, False, True, True, False, True])
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2, location=(0.0, 0.0, 0.3))
bpy.context.object.name = 'DTopo'
bpy.ops.object.shade_flat()
bpy.data.objects['DTopo'].data.attributes.new(name='freestyle_face', type='BOOLEAN', domain='FACE')
bpy.data.objects['DTopo'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DTopo'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DTopo'].modifiers['DT'].use_poly_data = True
bpy.data.objects['DTopo'].modifiers['DT'].data_types_polys = {'SMOOTH', 'FREESTYLE_FACE'}
bpy.data.objects['DTopo'].modifiers['DT'].poly_mapping = 'TOPOLOGY'
bpy.data.objects['DTopo'].modifiers['DT'].use_object_transform = False
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DNearest'
bpy.ops.object.shade_flat()
bpy.data.objects['DNearest'].data.attributes.new(name='freestyle_face', type='BOOLEAN', domain='FACE')
bpy.data.objects['DNearest'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNearest'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DNearest'].modifiers['DT'].use_poly_data = True
bpy.data.objects['DNearest'].modifiers['DT'].data_types_polys = {'SMOOTH', 'FREESTYLE_FACE'}
bpy.data.objects['DNearest'].modifiers['DT'].poly_mapping = 'NEAREST'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DNormal'
bpy.ops.object.shade_flat()
bpy.data.objects['DNormal'].data.attributes.new(name='freestyle_face', type='BOOLEAN', domain='FACE')
bpy.data.objects['DNormal'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNormal'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DNormal'].modifiers['DT'].use_poly_data = True
bpy.data.objects['DNormal'].modifiers['DT'].data_types_polys = {'SMOOTH', 'FREESTYLE_FACE'}
bpy.data.objects['DNormal'].modifiers['DT'].poly_mapping = 'NORMAL'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DNormalRay'
bpy.ops.object.shade_flat()
bpy.data.objects['DNormalRay'].data.attributes.new(name='freestyle_face', type='BOOLEAN', domain='FACE')
bpy.data.objects['DNormalRay'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNormalRay'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DNormalRay'].modifiers['DT'].use_poly_data = True
bpy.data.objects['DNormalRay'].modifiers['DT'].data_types_polys = {'SMOOTH', 'FREESTYLE_FACE'}
bpy.data.objects['DNormalRay'].modifiers['DT'].poly_mapping = 'NORMAL'
bpy.data.objects['DNormalRay'].modifiers['DT'].ray_radius = 0.05
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DPnorproj'
bpy.ops.object.shade_flat()
bpy.data.objects['DPnorproj'].data.attributes.new(name='freestyle_face', type='BOOLEAN', domain='FACE')
bpy.data.objects['DPnorproj'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DPnorproj'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DPnorproj'].modifiers['DT'].use_poly_data = True
bpy.data.objects['DPnorproj'].modifiers['DT'].data_types_polys = {'SMOOTH', 'FREESTYLE_FACE'}
bpy.data.objects['DPnorproj'].modifiers['DT'].poly_mapping = 'POLYINTERP_PNORPROJ'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DPnorprojRay'
bpy.ops.object.shade_flat()
bpy.data.objects['DPnorprojRay'].data.attributes.new(name='freestyle_face', type='BOOLEAN', domain='FACE')
bpy.data.objects['DPnorprojRay'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DPnorprojRay'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DPnorprojRay'].modifiers['DT'].use_poly_data = True
bpy.data.objects['DPnorprojRay'].modifiers['DT'].data_types_polys = {'SMOOTH', 'FREESTYLE_FACE'}
bpy.data.objects['DPnorprojRay'].modifiers['DT'].poly_mapping = 'POLYINTERP_PNORPROJ'
bpy.data.objects['DPnorprojRay'].modifiers['DT'].ray_radius = 0.03
