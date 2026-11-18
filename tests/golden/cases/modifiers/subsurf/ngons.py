# WP 4.4: SUBSURF on faces other than quads: the cylinder's hexagon caps, the cone's triangles
# and pentagon (one ptex face per corner at half resolution), and an n-gon disc (every vertex
# a smooth boundary corner).
# golden: startup=empty compare=evaluated:Cylinder,evaluated:Cone,evaluated:Circle tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=6)
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON', location=(6, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cone'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cone'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Circle'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Circle'].modifiers['Subdivision'].levels = 2
