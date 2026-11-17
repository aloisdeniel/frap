# WP 1.26: `bpy.context` is read-only (code CONTEXT_READONLY, AttributeError). Blender has two
# wordings: "Context property … is read-only" for members computed by the context callback
# (object, active_object, selected_objects, …) and "attribute … from "Context" is read-only" for
# RNA properties of Context (scene, view_layer, mode, …). Writes through context members reach
# the data they point at; the context itself is unchanged.
# golden: startup=factory tier=T0 compare=context,objects
import bpy
bpy.context.object = bpy.data.objects['Camera']
bpy.context.active_object = None
bpy.context.selected_objects = []
bpy.context.edit_object = None
bpy.context.selected_editable_objects = []
bpy.context.visible_objects = []
bpy.context.scene = bpy.data.scenes['Scene']
bpy.context.view_layer = None
bpy.context.collection = None
bpy.context.mode = 'EDIT_MESH'
bpy.context.window = None
bpy.context.preferences = None
bpy.context.blend_data = None
bpy.context.object.location = (1, 2, 3)
bpy.context.scene.cursor.location = (0, 0, 1)
