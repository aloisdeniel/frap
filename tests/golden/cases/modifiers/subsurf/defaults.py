# WP 4.4: SUBSURF with its defaults (Catmull-Clark, viewport level 1, limit surface, quality 3,
# creases on, boundary ALL, uv_smooth PRESERVE_BOUNDARIES) on the factory cube: Blender's
# vertex/edge/face/corner order, limit positions, evaluated UVs, flat shading kept.
# golden: startup=factory compare=evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
