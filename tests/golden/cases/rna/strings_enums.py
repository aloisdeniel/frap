# WP 1.26: string and enum values read back through object names (the dumped keys): enum
# reads after writes, `bpy.context.mode`, the render engine, a light's type, an empty name
# and non-ASCII names, a name read from a pointer and from a node.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].rotation_mode = 'ZXY'
bpy.data.objects['Cube'].name = bpy.data.objects['Cube'].rotation_mode
bpy.data.scenes['Scene'].render.engine = 'BLENDER_WORKBENCH'
bpy.data.objects['Light'].name = bpy.data.scenes['Scene'].render.engine
bpy.data.objects['Camera'].name = bpy.context.mode
bpy.data.objects['OBJECT'].name = bpy.data.lights['Light'].type
bpy.data.objects['ZXY'].name = 'Ünïcødé ✓'
bpy.data.objects['POINT'].name = ''
bpy.data.objects['BLENDER_WORKBENCH'].name = bpy.data.objects['Ünïcødé ✓'].data.name
bpy.data.objects['Cube'].name = bpy.data.objects['Ünïcødé ✓'].active_material.node_tree.nodes[1].name
bpy.data.objects[0].name
bpy.data.objects[1].name
bpy.data.objects[2].name
