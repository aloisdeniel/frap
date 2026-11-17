# WP 1.26: writes of read-only properties (code RNA_READONLY, AttributeError): enums, ints,
# strings, pointers and collections of IDs and of `bpy.data`. Nothing changes.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].type = 'EMPTY'
bpy.data.objects['Cube'].mode = 'EDIT'
bpy.data.objects['Cube'].users = 2
bpy.data.meshes['Cube'].users = 2
bpy.data.objects['Cube'].name_full = 'x'
bpy.data.objects['Cube'].bound_box = None
bpy.data.objects['Cube'].data.vertices = None
bpy.data.scenes['Scene'].objects = None
bpy.data.objects = None
bpy.data.objects['Cube'].material_slots = None
