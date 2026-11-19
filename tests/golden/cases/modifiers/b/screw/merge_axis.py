# WP 4.3: SCREW with `use_merge_vertices` on a plane whose edge lies on the axis: the axis
# vertices weld into the first slice (quads become triangles); normals calculated.
# golden: startup=empty compare=evaluated:Plane tier=T1
import bpy
bpy.ops.mesh.primitive_plane_add(size=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0.5, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Plane'].modifiers.new(name='M', type='SCREW')
bpy.data.objects['Plane'].modifiers['M'].use_merge_vertices = True
bpy.data.objects['Plane'].modifiers['M'].use_normal_calculate = True
bpy.data.objects['Plane'].modifiers['M'].steps = 12
