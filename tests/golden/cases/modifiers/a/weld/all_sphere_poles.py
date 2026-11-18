# WP 4.2: WELD (ALL) on a UV sphere with a threshold that merges the ring next to each pole into
# the pole: the pole triangles collapse, the quads of the next ring become triangles. Canonical
# order: Blender's UV sphere edge and face order changes between runs (deviation D-19).
# golden: startup=empty compare=evaluated:Sphere tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=6, radius=1)
bpy.data.objects['Sphere'].modifiers.new(name='Weld', type='WELD')
bpy.data.objects['Sphere'].modifiers['Weld'].merge_threshold = 0.55
