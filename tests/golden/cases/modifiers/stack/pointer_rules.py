# D-97 (M3+M4 stabilization round 2): modifier `Object` pointers are `PROP_ID_SELF_CHECK`
# (the modifier's own object raises TypeError before the setter runs), and the setters of
# `RNA_MOD_OBJECT_SET` refuse objects of another type silently: an ARRAY cap or a BOOLEAN
# operand that is a camera stays None, so the ARRAY still evaluates (16 vertices).
# golden: startup=factory compare=evaluated:Cube,objects tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube'].modifiers.new(name='Cast', type='CAST')
bpy.data.objects['Cube'].modifiers.new(name='Screw', type='SCREW')
bpy.data.objects['Cube'].modifiers.new(name='Deform', type='SIMPLE_DEFORM')
bpy.data.objects['Cube'].modifiers.new(name='Normals', type='NORMAL_EDIT')
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['Array'].offset_object = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Array'].start_cap = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Array'].start_cap = bpy.data.objects['Camera']
bpy.data.objects['Cube'].modifiers['Array'].end_cap = bpy.data.objects['Light']
bpy.data.objects['Cube'].modifiers['Mirror'].mirror_object = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Cast'].object = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Screw'].object = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Deform'].origin = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Normals'].target = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Camera']
bpy.data.objects['Cube'].modifiers['Displace'].texture_coords_object = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers['Cast'].object = bpy.data.objects['Camera']
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Mirror'])
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Cast'])
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Screw'])
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Deform'])
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Normals'])
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Boolean'])
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Displace'])
