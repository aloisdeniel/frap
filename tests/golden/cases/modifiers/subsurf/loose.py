# WP 4.4: SUBSURF with loose geometry: loose vertices keep their position, loose edges are
# subdivided along a cubic B-spline through their neighbours (their end points move too), and
# vertices of loose edges are infinitely sharp corners of the surface.
# golden: startup=empty compare=evaluated:Loose tier=T1
import bpy
bpy.data.meshes.new('Loose')
bpy.data.meshes['Loose'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0.3), (0, 1, 0), (2, 0, 0), (3, 0.5, 0), (4, 0, 1), (5, 5, 5), (2, 2, 0)], [(1, 4), (4, 5), (5, 6), (2, 8)], [(0, 1, 2, 3)])
bpy.data.objects.new('Loose', bpy.data.meshes['Loose'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Loose'])
bpy.data.objects['Loose'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Loose'].modifiers['Subdivision'].levels = 2
