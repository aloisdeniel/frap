# WP 1.19 (T0), object.select_all: a visible but unselectable object (hide_select) is not
# selected, yet SELECT finishes (Blender counts it as a change); with every base hidden by its
# layer collection nothing is visible and the operator passes through, keeping the selection.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Camera'].hide_select = True
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.select_all(action='SELECT')
bpy.data.objects['Camera'].hide_select = False
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = True
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_all(action='INVERT')
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = False
