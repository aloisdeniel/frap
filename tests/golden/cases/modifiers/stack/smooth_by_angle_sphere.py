# WP 4.1 (T2): "Smooth by Angle" on UV spheres, compared canonically (Blender's UV sphere edge
# and face order changes between runs, D-19): ring edges (11.25 degrees) sharp at 0.15 rad,
# segment edges sharp near the equator only; 12x6 at 0.45 rad.
# golden: startup=factory compare=evaluated:Sphere,evaluated:Sphere.001 tier=T2 order=canonical
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(location=(3, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=0.15)
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(6, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=0.45)
