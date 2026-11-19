# WP 4.4: SUBSURF creases: crease_edge 1.0 (infinitely sharp), 0.5 and 0.3 (semi-sharp, decaying
# before the isolation level), crease_vert 1.0 and 0.4; use_creases=False ignores them on a
# copy. crease_edge is copied to the edges along each coarse edge, crease_vert interpolated.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.data.meshes['Cube'].attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.meshes['Cube'].attributes['crease_edge'].data[0].value = 1.0
bpy.data.meshes['Cube'].attributes['crease_edge'].data[1].value = 1.0
bpy.data.meshes['Cube'].attributes['crease_edge'].data[2].value = 1.0
bpy.data.meshes['Cube'].attributes['crease_edge'].data[3].value = 1.0
bpy.data.meshes['Cube'].attributes['crease_edge'].data[5].value = 0.5
bpy.data.meshes['Cube'].attributes['crease_edge'].data[7].value = 0.3
bpy.data.meshes['Cube'].attributes.new(name='crease_vert', type='FLOAT', domain='POINT')
bpy.data.meshes['Cube'].attributes['crease_vert'].data[6].value = 1.0
bpy.data.meshes['Cube'].attributes['crease_vert'].data[5].value = 0.4
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.001'].modifiers['Subdivision'].use_creases = False
