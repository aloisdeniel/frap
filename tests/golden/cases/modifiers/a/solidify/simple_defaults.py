# WP 4.2: SOLIDIFY "Simple" (EXTRUDE) with its defaults: 0.01 thick, offset −1 (inwards); the
# closed cube gets a flipped inner shell and no rim; the plane gets a shell and four rim quads
# (the result always has `material_index`).
# golden: startup=factory compare=evaluated:Cube,evaluated:Plane tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.objects['Plane'].modifiers.new(name='Solidify', type='SOLIDIFY')
