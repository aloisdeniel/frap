# WP 7.7: natural-sort ties of the export tree. Roots whose natural keys are equal (`b001`,
# `B01`, `B1`, `b1`: the number 1 after a lower-cased `b`) spread over the master collection
# and two child collections, linked in an order that differs from bpy.data's; an empty `P`
# with tied children (`C01`, `c1`, `c001`). The add-on sorts `scene_eval.objects` (the
# scene's objects: master collection first, then the child collections depth first) with a
# stable sort, and the children from `bpy.data.objects`. The node order is not part of the
# canonical GLB, so the export is imported again into an empty scene (the importer links the
# objects in node order) and `P_Order` records where each one landed in the collection.
# golden: startup=factory tier=T2 tol=1e-6 compare=objects,glb:out/ties.glb glb_structure
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.objects.remove(bpy.data.objects['Camera'])
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.data.collections.new('C1')
bpy.context.scene.collection.children.link(bpy.data.collections['C1'])
bpy.data.collections.new('C2')
bpy.context.scene.collection.children.link(bpy.data.collections['C2'])
bpy.data.collections.new('C3')
bpy.data.collections['C1'].children.link(bpy.data.collections['C3'])
bpy.data.objects.new('b1', None)
bpy.data.collections['C2'].objects.link(bpy.data.objects['b1'])
bpy.data.objects.new('B01', None)
bpy.data.collections['C3'].objects.link(bpy.data.objects['B01'])
bpy.data.objects.new('b001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['b001'])
bpy.data.objects.new('B1', None)
bpy.data.collections['C1'].objects.link(bpy.data.objects['B1'])
bpy.data.objects.new('a', None)
bpy.data.collections['C2'].objects.link(bpy.data.objects['a'])
bpy.data.objects.new('P', None)
bpy.data.collections['C1'].objects.link(bpy.data.objects['P'])
bpy.data.objects.new('c1', None)
bpy.data.collections['C1'].objects.link(bpy.data.objects['c1'])
bpy.data.objects.new('C01', None)
bpy.data.collections['C2'].objects.link(bpy.data.objects['C01'])
bpy.data.objects.new('c001', None)
bpy.data.collections['C2'].objects.link(bpy.data.objects['c001'])
bpy.data.objects['c1'].parent = bpy.data.objects['P']
bpy.data.objects['C01'].parent = bpy.data.objects['P']
bpy.data.objects['c001'].parent = bpy.data.objects['P']
bpy.data.objects['B1'].location = (1.0, 0.0, 0.0)
bpy.data.objects['b1'].location = (0.0, 2.0, 0.0)
bpy.ops.export_scene.gltf(filepath='out/ties.glb')
bpy.data.objects.remove(bpy.data.objects['b1'])
bpy.data.objects.remove(bpy.data.objects['B01'])
bpy.data.objects.remove(bpy.data.objects['b001'])
bpy.data.objects.remove(bpy.data.objects['B1'])
bpy.data.objects.remove(bpy.data.objects['a'])
bpy.data.objects.remove(bpy.data.objects['P'])
bpy.data.objects.remove(bpy.data.objects['c1'])
bpy.data.objects.remove(bpy.data.objects['C01'])
bpy.data.objects.remove(bpy.data.objects['c001'])
bpy.ops.import_scene.gltf(filepath='out/ties.glb')
bpy.data.objects.new('P_Order', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Order'])
bpy.data.objects['P_Order'].location = (bpy.context.collection.objects.find('a'), bpy.context.collection.objects.find('b001'), bpy.context.collection.objects.find('B01'))
bpy.data.objects['P_Order'].scale = (bpy.context.collection.objects.find('B1'), bpy.context.collection.objects.find('b1'), bpy.context.collection.objects.find('P'))
bpy.data.objects['P_Order'].rotation_euler = (bpy.context.collection.objects.find('C01'), bpy.context.collection.objects.find('c1'), bpy.context.collection.objects.find('c001'))
