# WP 1.21 (T0), object.parent_set types CURVE, LATTICE and ARMATURE with a mesh parent act as
# OBJECT.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.parent_set(type='CURVE')
bpy.data.objects['Light'].parent_type
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Light'].select_set(False)
bpy.ops.object.parent_set(type='ARMATURE')
bpy.data.objects['Camera'].parent_type
