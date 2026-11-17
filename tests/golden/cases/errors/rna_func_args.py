# WP 1.26: RNA function arguments (code RNA_FUNC_ARGS, TypeError): missing required parameters,
# optional parameters given positionally, unknown keywords, wrong types (positional and
# keyword), on object, `bpy.data` collection, material-list and node-link functions.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.data.objects['Cube'].select_set()
bpy.data.objects['Cube'].select_set(True, 1)
bpy.data.objects['Cube'].select_set(True, view_layer=1)
bpy.data.objects['Cube'].select_set(state=True, foo=1)
bpy.data.objects['Cube'].select_set('yes')
bpy.data.objects['Cube'].select_get(1)
bpy.data.objects['Cube'].hide_set()
bpy.data.objects.new()
bpy.data.objects.new('X')
bpy.data.objects.new('X', 1)
bpy.data.objects.new(1, None)
bpy.data.materials.new()
bpy.data.meshes.new(name=2)
bpy.data.objects.remove(bpy.data.objects['Cube'], foo=1)
bpy.data.objects['Cube'].data.materials.append(bpy.data.objects['Camera'])
bpy.data.materials['Material'].node_tree.links.new(1, 2)
