# WP 8.3.13: falloff_type='RANDOM' seeds its RNG with BLI_ghashutil_strhash_p(object name),
# which reads the name as signed chars: non-ASCII (UTF-8 bytes >= 0x80) names sign-extend.
# VERTEX_WEIGHT_EDIT and VERTEX_WEIGHT_PROXIMITY with RANDOM on objects named 'Ébauche' and
# 'Prøximité', next to the ASCII 'Ebauche' (a different seed).
# golden: startup=empty compare=evaluated:Ébauche,evaluated:Ebauche,evaluated:Prøximité tier=T2
import bpy
bpy.ops.mesh.primitive_plane_add(size=1.2, location=(0.3, 0.2, 0.8), rotation=(0.4, 0.2, 0.0))
bpy.context.object.name = 'T'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'Ébauche'
bpy.data.objects['Ébauche'].vertex_groups.new(name='G')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([1, 6, 19, 20, 46, 49], 0.1, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([9, 56, 58, 61], 0.2, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([21, 34], 0.3, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([45, 52, 55], 0.4, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([15, 26, 50], 0.5, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([29, 33, 36, 43, 47, 54], 0.6, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([0, 11, 12, 60, 62, 63], 0.7, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([4, 14, 41, 48], 0.8, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([3, 13, 16, 32, 59], 0.9, 'REPLACE')
bpy.data.objects['Ébauche'].vertex_groups['G'].add([8, 30, 35, 57], 1.0, 'REPLACE')
bpy.data.objects['Ébauche'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['Ébauche'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['Ébauche'].modifiers['W'].falloff_type = 'RANDOM'
bpy.data.objects['Ébauche'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['Ébauche'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['Ébauche'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['Ébauche'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['Ébauche'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'Ebauche'
bpy.data.objects['Ebauche'].vertex_groups.new(name='G')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([1, 6, 19, 20, 46, 49], 0.1, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([9, 56, 58, 61], 0.2, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([21, 34], 0.3, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([45, 52, 55], 0.4, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([15, 26, 50], 0.5, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([29, 33, 36, 43, 47, 54], 0.6, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([0, 11, 12, 60, 62, 63], 0.7, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([4, 14, 41, 48], 0.8, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([3, 13, 16, 32, 59], 0.9, 'REPLACE')
bpy.data.objects['Ebauche'].vertex_groups['G'].add([8, 30, 35, 57], 1.0, 'REPLACE')
bpy.data.objects['Ebauche'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['Ebauche'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['Ebauche'].modifiers['W'].falloff_type = 'RANDOM'
bpy.data.objects['Ebauche'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['Ebauche'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['Ebauche'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['Ebauche'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['Ebauche'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'Prøximité'
bpy.data.objects['Prøximité'].vertex_groups.new(name='G')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([1, 6, 19, 20, 46, 49], 0.1, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([9, 56, 58, 61], 0.2, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([21, 34], 0.3, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([45, 52, 55], 0.4, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([15, 26, 50], 0.5, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([29, 33, 36, 43, 47, 54], 0.6, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([0, 11, 12, 60, 62, 63], 0.7, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([4, 14, 41, 48], 0.8, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([3, 13, 16, 32, 59], 0.9, 'REPLACE')
bpy.data.objects['Prøximité'].vertex_groups['G'].add([8, 30, 35, 57], 1.0, 'REPLACE')
bpy.data.objects['Prøximité'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['Prøximité'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['Prøximité'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['Prøximité'].modifiers['W'].proximity_geometry = {'FACE'}
bpy.data.objects['Prøximité'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['Prøximité'].modifiers['W'].max_dist = 1.5
bpy.data.objects['Prøximité'].modifiers['W'].falloff_type = 'RANDOM'
bpy.data.objects['Prøximité'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['Prøximité'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['Prøximité'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['Prøximité'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['Prøximité'].modifiers['Dz'].strength = 1.0
