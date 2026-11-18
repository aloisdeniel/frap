# WP 3.10 (M34W0-4): `object.make_links_data(type=…)` from the active object to the other
# selected objects: MATERIAL (slots grow, materials go where the target's active slot links),
# MODIFIERS (none to copy here: the copies are unit-tested), OBDATA (same type only; slots follow the data),
# GROUPS (the target leaves its collections and joins the source's), ANIMATION and the types
# frap objects never use (no change); poll: an active object in Object Mode.
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4)
bpy.context.object.data.materials.append(bpy.data.materials.new('Red'))
bpy.context.object.data.materials.append(bpy.data.materials.new('Blue'))
bpy.data.collections.new('Extra')
bpy.context.scene.collection.children.link(bpy.data.collections['Extra'])
bpy.data.collections['Extra'].objects.link(bpy.context.object)
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.make_links_data(type='MATERIAL')
bpy.ops.object.make_links_data(type='MODIFIERS')
bpy.ops.object.make_links_data(type='GROUPS')
bpy.ops.object.make_links_data(type='ANIMATION')
bpy.ops.object.make_links_data(type='DUPLICOLLECTION')
bpy.ops.object.make_links_data(type='CONSTRAINTS')
bpy.ops.object.make_links_data(type='FONTS')
bpy.ops.object.make_links_data(type='EFFECTS')
bpy.ops.object.make_links_data(type='LIGHT_LINKING')
bpy.ops.object.make_links_data(type='SHADOW_LINKING')
bpy.ops.object.make_links_data(type='OBDATA')
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].data.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].users_collection[0].name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Camera'].users_collection[0].name, None))
bpy.data.objects['Extra'].location = (bpy.data.meshes['Cube'].users, bpy.data.materials['Red'].users, bpy.data.materials['Material'].users)
bpy.data.objects['Camera'].select_set(False)
bpy.ops.object.make_links_data(type='CAMERA')
bpy.context.view_layer.objects.active = None
bpy.ops.object.make_links_data(type='OBDATA')
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.make_links_data(type='OBDATA')
bpy.ops.object.make_links_data(type='MATERIAL')
bpy.ops.object.make_links_data(type='GROUPS')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Material'])
bpy.context.object.material_slots[0].link = 'OBJECT'
bpy.context.object.material_slots[0].material = bpy.data.materials['Red']
bpy.data.objects['Sphere'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Sphere']
bpy.ops.object.make_links_data(type='MATERIAL')
bpy.data.objects['Plane'].material_slots[1].link
