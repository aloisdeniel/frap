# WP 8.3.4: object.hide_render_clear_all (bl_operators/object.py) sets hide_render = False on
# every object of the scene, also in Edit Mode (no poll); a collection's hide_render stays, so
# its objects remain unrenderable. The renderable objects are read back through a glTF export
# with use_renderable=True.
# golden: startup=factory compare=objects,glb:out/renderable.glb tier=T0
import bpy
bpy.data.collections.new('Hidden')
bpy.context.scene.collection.children.link(bpy.data.collections['Hidden'])
bpy.data.collections['Hidden'].objects.link(bpy.data.objects.new('Inner', None))
bpy.data.collections['Hidden'].hide_render = True
bpy.data.objects['Cube'].hide_render = True
bpy.data.objects['Light'].hide_render = True
bpy.data.objects['Inner'].hide_render = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.hide_render_clear_all()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/renderable.glb', use_renderable=True)
