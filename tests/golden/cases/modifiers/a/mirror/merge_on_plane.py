# WP 4.2: MIRROR merge: the cube moved so its -X face lies on the mirror plane; the four
# vertices on the plane merge with their images (the copy into the original, averaged), the
# coincident faces collapse into one. A second object keeps `use_mirror_merge` off.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube.001'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube.001'].modifiers['Mirror'].use_mirror_merge = False
