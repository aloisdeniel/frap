# WP 4.2: MIRROR bisect: the cube is cut along each mirror plane (BMesh bisect, vertices beyond
# the plane deleted) before mirroring; X with bisect, Y with a flipped bisect, Z without, so the
# axes are applied in order X, Y, Z and the cut vertices on the planes merge.
# golden: startup=factory compare=evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].location = (0.3, 0, 0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0.25, -0.4, 0.1))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube'].modifiers['Mirror'].use_axis = (True, True, True)
bpy.data.objects['Cube'].modifiers['Mirror'].use_bisect_axis = (True, True, False)
bpy.data.objects['Cube'].modifiers['Mirror'].use_bisect_flip_axis = (False, True, False)
