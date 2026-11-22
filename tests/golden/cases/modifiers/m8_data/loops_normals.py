# WP 8.3.13: DATA_TRANSFER of custom normals
# with every loop_mapping and the mix modes, from a smooth ico sphere with
# sharp edges to a rotated smooth torus inside it (TOPOLOGY: the same ico sphere); a mask group.
# Custom normals encode within one INT16 unit (D-122): tol 3e-3.
# golden: startup=empty compare=evaluated:DTopo,evaluated:DNearNormal,evaluated:DPolyNor,evaluated:DNearPoly,evaluated:DInterp,evaluated:DLnorproj,evaluated:DMix,evaluated:DAdd,evaluated:DSub,evaluated:DMul,evaluated:DMask tier=T2 tol=3e-3
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1.0)
bpy.context.object.name = 'S'
bpy.ops.object.shade_smooth()
bpy.data.objects['S'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes['sharp_edge'].data.foreach_set('value', [False, False, False, False, False, False, False, False, False, False, False, False, True, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, True, False, False, False, False, True, False, True, True, False, True, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, False, True, False, False, False, False, False, False, False, False, False, False, True, False, False, True, False, False, False, True, False, False, True, False, False, False, False, False, False, True, False, False, False, False, False, False, True, False, False, False, False, False, False, True, False, False, False, True, False, False])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1.0)
bpy.context.object.name = 'DTopo'
bpy.data.objects['DTopo'].location = (0.0, 0.0, 0.2)
bpy.ops.object.shade_smooth()
bpy.data.objects['DTopo'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DTopo'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DTopo'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DTopo'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DTopo'].modifiers['DT'].loop_mapping = 'TOPOLOGY'
bpy.data.objects['DTopo'].modifiers['DT'].use_object_transform = False
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DNearNormal'
bpy.ops.object.shade_smooth()
bpy.data.objects['DNearNormal'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNearNormal'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DNearNormal'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DNearNormal'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DNearNormal'].modifiers['DT'].loop_mapping = 'NEAREST_NORMAL'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DPolyNor'
bpy.ops.object.shade_smooth()
bpy.data.objects['DPolyNor'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DPolyNor'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DPolyNor'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DPolyNor'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DPolyNor'].modifiers['DT'].loop_mapping = 'NEAREST_POLYNOR'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DNearPoly'
bpy.ops.object.shade_smooth()
bpy.data.objects['DNearPoly'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNearPoly'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DNearPoly'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DNearPoly'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DNearPoly'].modifiers['DT'].loop_mapping = 'NEAREST_POLY'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DInterp'
bpy.ops.object.shade_smooth()
bpy.data.objects['DInterp'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DInterp'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DInterp'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DInterp'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DInterp'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DLnorproj'
bpy.ops.object.shade_smooth()
bpy.data.objects['DLnorproj'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DLnorproj'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DLnorproj'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DLnorproj'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DLnorproj'].modifiers['DT'].loop_mapping = 'POLYINTERP_LNORPROJ'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DMix'
bpy.ops.object.shade_smooth()
bpy.data.objects['DMix'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMix'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMix'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DMix'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DMix'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMix'].modifiers['DT'].mix_mode = 'MIX'
bpy.data.objects['DMix'].modifiers['DT'].mix_factor = 0.5
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DAdd'
bpy.ops.object.shade_smooth()
bpy.data.objects['DAdd'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DAdd'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DAdd'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DAdd'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DAdd'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DAdd'].modifiers['DT'].mix_mode = 'ADD'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DSub'
bpy.ops.object.shade_smooth()
bpy.data.objects['DSub'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DSub'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DSub'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DSub'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DSub'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DSub'].modifiers['DT'].mix_mode = 'SUB'
bpy.data.objects['DSub'].modifiers['DT'].mix_factor = 0.7
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DMul'
bpy.ops.object.shade_smooth()
bpy.data.objects['DMul'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMul'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMul'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DMul'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DMul'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMul'].modifiers['DT'].mix_mode = 'MUL'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DMask'
bpy.ops.object.shade_smooth()
bpy.data.objects['DMask'].vertex_groups.new(name='M')
bpy.data.objects['DMask'].vertex_groups['M'].add([0, 4, 32, 34, 37, 54, 76, 81, 82, 83, 91, 104, 113, 114, 119, 127], 0.1, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([5, 6, 16, 19, 21, 58, 79, 86, 99, 111, 117], 0.2, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([14, 47, 60, 92, 107, 122], 0.3, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([17, 24, 35, 66, 85, 98, 118], 0.4, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([12, 23, 25, 28, 29, 45, 52, 56, 61, 90, 100, 115, 116], 0.5, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([1, 3, 10, 11, 20, 22, 27, 30, 31, 33, 38, 40, 42, 43, 48, 71, 88, 89, 93, 101, 109, 112, 123], 0.6, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([7, 8, 9, 15, 36, 57, 59, 105, 124], 0.7, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([26, 50, 53, 55, 67, 72, 74, 75, 77, 78, 80, 87, 103, 110, 120, 121], 0.8, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([2, 46, 49, 51, 96, 106, 108], 0.9, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([39, 64, 94, 95, 97], 1.0, 'REPLACE')
bpy.data.objects['DMask'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMask'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMask'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DMask'].modifiers['DT'].data_types_loops = {'CUSTOM_NORMAL'}
bpy.data.objects['DMask'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMask'].modifiers['DT'].vertex_group = 'M'
