# WP 1.22: `ID.rename` (its return values made visible by assigning them as material names),
# `ID.copy` (lowest free `.NNN`, shared data, kept parent and slot overrides, linked nowhere,
# a material copy with its own tree, fake user cleared) and `ID.user_clear`. The object copies
# are linked at the end so that Blender evaluates their `matrix_world`; their source is
# deselected first (Blender's copy remembers the selection for its future base, frap does not).
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.data.materials.new('M1')
bpy.data.materials.new('M2')
bpy.data.materials.new('M3')
bpy.data.materials.new('M4')
bpy.data.materials.new('M5')
bpy.data.objects.new('R', None)
bpy.data.objects.new('R.002', None)
bpy.data.materials['M1'].name = bpy.data.objects['R'].rename('R')
bpy.data.materials['M2'].name = bpy.data.objects['R'].rename('R.002')
bpy.data.materials['M3'].name = bpy.data.objects['R.001'].rename('R.002', mode='SAME_ROOT')
bpy.data.materials['M4'].name = bpy.data.objects['Camera'].rename('Light', mode='ALWAYS')
bpy.data.materials['M5'].name = bpy.data.objects['Cube'].rename('')
bpy.data.objects['Light'].rename('Foo', mode='BAD')
bpy.data.objects['Light'].rename()
bpy.data.objects['Light'].rename(1)
bpy.data.materials['Material'].node_tree.rename('Tree')
bpy.data.materials['Material'].node_tree.name
bpy.context.scene.collection.rename('Top')
bpy.data.objects['Object'].parent = bpy.data.objects['Light.001']
bpy.data.objects['Object'].location = (1, 2, 3)
bpy.data.objects['Object'].material_slots[0].link = 'OBJECT'
bpy.data.objects['Object'].material_slots[0].material = bpy.data.materials['UNCHANGED']
bpy.data.objects['Object'].select_set(False)
bpy.data.objects['Object'].copy()
bpy.data.objects['Object'].copy().name
bpy.data.materials['Material'].copy()
bpy.data.materials['Material'].copy().node_tree.nodes.new('ShaderNodeMath')
bpy.data.meshes['Cube'].copy()
bpy.data.objects.new('Holder', bpy.data.meshes['Cube.001'])
bpy.data.collections['Collection'].copy()
bpy.data.cameras['Camera'].copy()
bpy.data.lights['Light'].copy()
bpy.data.worlds['World'].copy()
bpy.data.images.new('Img', 2, 2).copy()
bpy.data.materials['Material'].use_fake_user = True
bpy.data.materials['Material'].copy().use_fake_user
bpy.data.materials['Material'].user_clear()
bpy.data.objects['Object.002'].user_clear()
bpy.data.materials['Material'].node_tree.user_clear()
bpy.data.objects['Object'].copy(1)
bpy.context.scene.collection.objects.link(bpy.data.objects['Object.001'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Object.002'])
