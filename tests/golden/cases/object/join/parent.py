# WP 1.20 (T0), object.join and relations: the active Cube's parent (the plane) is joined into
# it, so the Cube is unparented keeping its world transform; the Light, a child of the joined
# plane, loses its parent and keeps its local transform; 'Twin' shares the Cube's mesh and is
# joined as separate geometry with its own transform, then deleted; one material per slot.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T0
import bpy
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 2))
bpy.data.objects['Cube'].parent = bpy.data.objects['Plane']
bpy.data.objects['Light'].parent = bpy.data.objects['Plane']
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Twin'])
bpy.data.objects['Twin'].location = (0, 5, 0)
bpy.data.objects['Twin'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.join()
bpy.data.objects['Twin']
bpy.data.objects['Light'].parent
