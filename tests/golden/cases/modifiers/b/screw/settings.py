# WP 4.3: SCREW on an edge-only circle: an open turn of 3 rad about X in 7 steps, twice, with a
# screw offset, flat shading, flipped normals and stretched UVs (none: no faces, no UVs).
# golden: startup=empty compare=evaluated:Circle tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=6, radius=0.5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 2, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Circle'].modifiers.new(name='M', type='SCREW')
bpy.data.objects['Circle'].modifiers['M'].axis = 'X'
bpy.data.objects['Circle'].modifiers['M'].angle = 3.0
bpy.data.objects['Circle'].modifiers['M'].steps = 7
bpy.data.objects['Circle'].modifiers['M'].iterations = 2
bpy.data.objects['Circle'].modifiers['M'].screw_offset = 1.0
bpy.data.objects['Circle'].modifiers['M'].use_smooth_shade = False
bpy.data.objects['Circle'].modifiers['M'].use_normal_flip = True
bpy.data.objects['Circle'].modifiers['M'].use_stretch_u = True
bpy.data.objects['Circle'].modifiers['M'].use_stretch_v = True
