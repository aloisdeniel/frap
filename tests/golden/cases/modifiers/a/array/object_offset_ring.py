# WP 4.2: ARRAY with an object offset: an empty rotated 45° about Z makes 8 copies of a plane
# around the origin whose inner corners touch; `use_merge_vertices_cap` welds the last copy to
# the first. A second object uses an offset object with scale (no translated mapping: every
# chunk is matched by sorting), so the shrinking copies come within the merge distance.
# golden: startup=empty compare=evaluated:Plane,evaluated:Plane.001 tier=T1
import bpy
bpy.ops.object.empty_add(rotation=(0, 0, 0.7853981633974483))
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(1.7071067811865475, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Plane'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Plane'].modifiers['Array'].count = 8
bpy.data.objects['Plane'].modifiers['Array'].use_relative_offset = False
bpy.data.objects['Plane'].modifiers['Array'].use_object_offset = True
bpy.data.objects['Plane'].modifiers['Array'].offset_object = bpy.data.objects['Empty']
bpy.data.objects['Plane'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Plane'].modifiers['Array'].use_merge_vertices_cap = True
bpy.data.objects['Plane'].modifiers['Array'].merge_threshold = 0.05
bpy.ops.object.empty_add(location=(4, 0, 0), scale=(0.8, 0.8, 0.8))
bpy.ops.mesh.primitive_plane_add(size=1, location=(4, 0, 0))
bpy.data.objects['Plane.001'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Plane.001'].modifiers['Array'].count = 5
bpy.data.objects['Plane.001'].modifiers['Array'].relative_offset_displace = (1, 0, 0)
bpy.data.objects['Plane.001'].modifiers['Array'].use_object_offset = True
bpy.data.objects['Plane.001'].modifiers['Array'].offset_object = bpy.data.objects['Empty.001']
bpy.data.objects['Plane.001'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Plane.001'].modifiers['Array'].merge_threshold = 0.3
