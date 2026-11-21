# WP 7.7: the natural sort counts every Unicode decimal digit (`re.split(r'(\d+)')` on a str
# and `int()`): roots `x10`, `x٣` (Arabic-Indic 3), `x2`, `x１` (full-width 1), `X𝟗`
# (mathematical 9) and `x١٠٠` (100) export as x１, x2, x٣, X𝟗, x10, x١٠٠. Imported again into
# an empty scene (the importer links the objects in node order); `P_Order` records where each
# one landed in the collection.
# golden: startup=factory tier=T2 tol=1e-6 compare=objects,glb:out/digits.glb glb_structure
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.objects.remove(bpy.data.objects['Camera'])
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.data.objects.new('x10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['x10'])
bpy.data.objects.new('x٣', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['x٣'])
bpy.data.objects.new('x2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['x2'])
bpy.data.objects.new('x１', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['x１'])
bpy.data.objects.new('X𝟗', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['X𝟗'])
bpy.data.objects.new('x١٠٠', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['x١٠٠'])
bpy.ops.export_scene.gltf(filepath='out/digits.glb')
bpy.data.objects.remove(bpy.data.objects['x10'])
bpy.data.objects.remove(bpy.data.objects['x٣'])
bpy.data.objects.remove(bpy.data.objects['x2'])
bpy.data.objects.remove(bpy.data.objects['x１'])
bpy.data.objects.remove(bpy.data.objects['X𝟗'])
bpy.data.objects.remove(bpy.data.objects['x١٠٠'])
bpy.ops.import_scene.gltf(filepath='out/digits.glb')
bpy.data.objects.new('P_Order', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Order'])
bpy.data.objects['P_Order'].location = (bpy.context.collection.objects.find('x１'), bpy.context.collection.objects.find('x2'), bpy.context.collection.objects.find('x٣'))
bpy.data.objects['P_Order'].scale = (bpy.context.collection.objects.find('X𝟗'), bpy.context.collection.objects.find('x10'), bpy.context.collection.objects.find('x١٠٠'))
