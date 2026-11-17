# WP 1.26: reading `bpy_prop_collection`s: whole collections, `keys()`/`values()`/`items()`,
# slices (with negative and open bounds), `get` with and without a default, `find`, and
# collections of nameless items. The reads are compared by outcome; the indices that `find`
# returns are written into the state.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects
bpy.data.objects.keys()
bpy.data.objects.values()
bpy.data.objects.items()
bpy.data.objects[0:2]
bpy.data.objects[1:]
bpy.data.objects[-2:]
bpy.data.objects[:]
bpy.data.objects.get('Nope')
bpy.data.meshes['Cube'].vertices
bpy.data.meshes['Cube'].polygons[1:3]
bpy.data.materials['Material'].node_tree.nodes.keys()
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs.keys()
bpy.context.scene.objects.keys()
bpy.context.view_layer.objects.keys()
bpy.data.objects['Cube'].location.x = bpy.data.objects.find('Light')
bpy.data.objects['Cube'].location.y = bpy.data.objects.find('Nope')
bpy.data.objects['Cube'].location.z = bpy.data.materials['Material'].node_tree.nodes.find('Principled BSDF')
bpy.data.objects['Cube'].scale.x = bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs.find('Metallic')
bpy.data.objects['Cube'].scale.y = bpy.context.view_layer.objects.find('Cube')
bpy.data.objects['Cube'].scale.z = bpy.data.collections['Collection'].objects.find('Camera')
