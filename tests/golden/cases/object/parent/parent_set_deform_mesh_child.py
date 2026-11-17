# WP 1.21 (T0), object.parent_set types LATTICE, CURVE and ARMATURE* with a mesh child: each
# parents like OBJECT and adds a Lattice/Curve/Armature modifier pointing at the parent, once
# per parent (a second LATTICE adds nothing, another parent adds Lattice.001).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.object.parent_set(type='LATTICE')
bpy.ops.object.parent_set(type='LATTICE')
bpy.ops.object.parent_set(type='CURVE')
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.ops.object.parent_set(type='ARMATURE_NAME')
bpy.data.objects['Cube'].modifiers['Lattice'].object.name
bpy.data.objects['Cube'].parent_type
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Light'].select_set(False)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.parent_set(type='LATTICE', keep_transform=True)
bpy.data.objects['Cube'].modifiers['Lattice.001'].object.name
