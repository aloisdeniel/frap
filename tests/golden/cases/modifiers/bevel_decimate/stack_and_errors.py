# WP 4.5: a BEVEL then DECIMATE stack on the cube; DECIMATE on a plane (one face) is a
# modifier error ("Modifier requires more than 3 input faces") and passes the mesh through;
# a zero-width BEVEL is disabled.
# golden: startup=factory compare=evaluated:Cube,evaluated:Plane,evaluated:Cube.001 tier=T2
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 2
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.25
bpy.data.objects['Cube'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Cube'].modifiers['Decimate'].ratio = 0.5
bpy.ops.mesh.primitive_plane_add(location=(0, 4, 0))
bpy.data.objects['Plane'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Plane'].modifiers['Decimate'].ratio = 0.5
bpy.ops.mesh.primitive_cube_add(location=(4, 0, 0))
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].width = 0
