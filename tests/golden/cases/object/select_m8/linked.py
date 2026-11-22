# WP 8.3.4: object.select_linked with OBDATA (a linked duplicate shares the mesh), MATERIAL (the
# active slot's material, in any slot of the others), DUPGROUP (the same instanced collection,
# compared only while instance_type is COLLECTION), LIBRARY / LIBRARY_OBDATA (every local
# visible object, those with data), extend; nothing to compare (PARTICLE, no data, no material,
# no instanced collection) cancels; no active object is an error. Each step's selection is
# recorded by a transform clear or a translation of the selected objects. (A cancelled call is
# only made with nothing selected: Blender keeps its deselect, frap rolls back, D-155.)
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.duplicate(linked=True)
bpy.context.object.name = 'Twin'
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(None)
bpy.context.object.data.materials.append(bpy.data.materials['Material'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.ops.object.empty_add(location=(1, 1, 1))
bpy.context.object.name = 'E'
bpy.data.collections.new('Src')
bpy.ops.object.collection_instance_add(collection='Src', location=(0, 4, 0))
bpy.ops.object.collection_instance_add(collection='Src', location=(0, 5, 0))
bpy.ops.object.collection_instance_add(collection='Collection', location=(0, 6, 0))
bpy.data.objects['Src.001'].instance_type = 'NONE'
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_linked()
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.select_linked(type='MATERIAL')
bpy.ops.transform.translate(value=(0, 0, 2))
bpy.data.objects['Plane'].select_set(True)
bpy.ops.object.select_linked(type='MATERIAL', extend=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Src']
bpy.ops.object.select_linked(type='DUPGROUP')
bpy.ops.transform.translate(value=(0, 0, 4))
bpy.context.view_layer.objects.active = bpy.data.objects['Src.001']
bpy.ops.object.select_linked(type='DUPGROUP', extend=True)
bpy.ops.transform.translate(value=(0, 0, 8))
bpy.ops.object.select_linked(type='LIBRARY')
bpy.ops.transform.translate(value=(0, 0, 16))
bpy.context.view_layer.objects.active = bpy.data.objects['Plane.001']
bpy.ops.object.select_linked(type='LIBRARY_OBDATA')
bpy.ops.transform.translate(value=(0, 0, 32))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_linked(type='PARTICLE')
bpy.ops.object.select_linked(type='MATERIAL')
bpy.context.view_layer.objects.active = bpy.data.objects['E']
bpy.ops.object.select_linked(type='OBDATA')
bpy.ops.object.select_linked(type='MATERIAL')
bpy.ops.object.select_linked(type='DUPGROUP')
bpy.ops.object.select_linked(type='LIBRARY_OBDATA')
bpy.ops.object.select_linked(type='LIBRARY')
bpy.context.view_layer.objects.active = None
bpy.ops.object.select_linked(extend=True)
bpy.ops.object.select_linked(type='MATERIALS')
