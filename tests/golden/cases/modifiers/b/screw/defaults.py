# WP 4.3: SCREW with its defaults (a full turn about Z in 16 steps, closed) on a plane moved
# off the axis in Edit Mode: the face strips keep the plane's UVs, offset along U.
# golden: startup=empty compare=evaluated:Plane tier=T1
import bpy
bpy.ops.mesh.primitive_plane_add(size=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(1.5, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Plane'].modifiers.new(name='M', type='SCREW')
