# WP 4.2: MIRROR on Suzanne: without bisect the symmetric halves overlap and the centre-line
# vertices merge; with bisect the left half is cut away first, then mirrored and merged. The
# bisect threshold snaps the centre-line vertices onto the plane.
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Suzanne.001 tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='Mirror', type='MIRROR')
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.data.objects['Suzanne.001'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Suzanne.001'].modifiers['Mirror'].use_bisect_axis[0] = True
bpy.data.objects['Suzanne.001'].modifiers['Mirror'].use_bisect_flip_axis[0] = True
bpy.data.objects['Suzanne.001'].modifiers['Mirror'].bisect_threshold = 0.01
