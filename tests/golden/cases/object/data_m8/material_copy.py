# WP 8.3.6 (T0): material.copy puts the context material (headless-extended, D-159: the active
# object's active material; Blender gets it from a #@blender temp_override(material=...)) into
# the copy buffer; material.paste replaces another material's node tree and display settings
# (colour, metallic, roughness, pass index, alpha threshold, blend method, backface culling;
# not surface_render_method) with the buffer's, relinking the pasted image by name (the
# name it had at copy time). Without a material slot the copy poll fails. Values are
# recorded in object transforms and in the names of new objects.
# golden: startup=factory compare=objects,materials tier=T0
import bpy
bpy.data.images.new('Grid', 8, 8)
bpy.data.materials['Material'].diffuse_color = (0.1, 0.2, 0.3, 0.5)
bpy.data.materials['Material'].metallic = 0.7
bpy.data.materials['Material'].roughness = 0.15
bpy.data.materials['Material'].pass_index = 4
bpy.data.materials['Material'].alpha_threshold = 0.25
bpy.data.materials['Material'].use_backface_culling = True
bpy.data.materials['Material'].surface_render_method = 'BLENDED'
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.1, 0.1, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Grid']
#@frap bpy.ops.material.copy()
#@blender with bpy.context.temp_override(material=bpy.context.object.active_material): assert bpy.ops.material.copy() == {'FINISHED'}
bpy.data.materials.new('Plain')
bpy.ops.object.material_slot_add()
bpy.context.object.material_slots[1].material = bpy.data.materials['Plain']
bpy.context.object.active_material_index = 1
#@frap bpy.ops.material.paste()
#@blender with bpy.context.temp_override(material=bpy.context.object.active_material): assert bpy.ops.material.paste() == {'FINISHED'}
bpy.data.objects['Camera'].location.x = bpy.data.materials['Plain'].metallic
bpy.data.objects['Camera'].location.y = bpy.data.materials['Plain'].roughness
bpy.data.objects['Camera'].location.z = bpy.data.materials['Plain'].alpha_threshold
bpy.data.objects['Light'].location.x = bpy.data.materials['Plain'].pass_index
bpy.data.objects['Light'].location.y = bpy.data.materials['Plain'].diffuse_color[2]
bpy.data.objects['Light'].location.z = bpy.data.materials['Plain'].diffuse_color[3]
bpy.data.objects.new(bpy.data.materials['Plain'].surface_render_method, None)
bpy.data.objects.new(bpy.data.materials['Plain'].blend_method, None)
bpy.data.materials['Plain'].use_backface_culling
bpy.data.objects.new(bpy.data.materials['Plain'].node_tree.nodes['Image Texture'].image.name, None)
bpy.data.images['Grid'].name = 'Renamed'
bpy.data.images.new('Grid', 4, 4)
bpy.data.materials.new('Third')
bpy.context.object.material_slots[1].material = bpy.data.materials['Third']
#@frap bpy.ops.material.paste()
#@blender with bpy.context.temp_override(material=bpy.context.object.active_material): assert bpy.ops.material.paste() == {'FINISHED'}
bpy.data.objects['Light'].scale.x = bpy.data.materials['Third'].node_tree.nodes['Image Texture'].image.size[0]
bpy.data.objects['Light'].scale.y = bpy.data.materials['Third'].pass_index
bpy.context.object.material_slots[1].material = None
bpy.ops.material.copy.poll()
bpy.ops.material.paste()
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.material.copy.poll()
bpy.ops.material.paste.poll()
