# WP 4.3: SCREW about a rotated, moved empty's Z axis, taking the screw offset from the
# empty's position along that axis (`use_object_screw_offset`).
# golden: startup=empty compare=evaluated:Plane tier=T1
import bpy
bpy.ops.object.empty_add(location=(0.2, 0.1, 0.6), rotation=(0.2, 0.1, 0))
bpy.ops.mesh.primitive_plane_add(size=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(1.5, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Plane'].modifiers.new(name='M', type='SCREW')
bpy.data.objects['Plane'].modifiers['M'].object = bpy.data.objects['Empty']
bpy.data.objects['Plane'].modifiers['M'].use_object_screw_offset = True
bpy.data.objects['Plane'].modifiers['M'].steps = 8
