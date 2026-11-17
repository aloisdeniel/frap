# WP 1.22: `bpy.data.<coll>.new` for cameras, lights (every type), worlds, images, meshes and
# collections, their argument errors, and objects made from the new data (type, data users).
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.data.cameras.new('Cam')
bpy.data.cameras.new('Cam').lens
bpy.data.cameras.new()
bpy.data.lights.new('Point', 'POINT').energy
bpy.data.lights.new('Sun', 'SUN').type
bpy.data.lights.new('Spot', type='SPOT').shadow_soft_size
bpy.data.lights.new('Area', 'AREA')
bpy.data.lights.new('Lamp')
bpy.data.lights.new('Lamp', type='FOO')
bpy.data.lights.new('Lamp', 'POINT', 1)
bpy.data.worlds.new('Sky')
bpy.data.worlds['Sky'].node_tree.nodes.keys()
bpy.data.worlds['Sky'].node_tree.name
bpy.data.worlds.new('Sky').users
bpy.data.images.new('Img', 4, 2)
bpy.data.images['Img'].size
bpy.data.images.new('Img', 0, 2).size
bpy.data.images.new('Img', 4, 4, float_buffer=True).colorspace_settings.name
bpy.data.images.new('Img', 4, 4, is_data=True).colorspace_settings.name
bpy.data.images.new('Img', 4, 4, float_buffer=True, is_data=True).colorspace_settings.name
bpy.data.images.new('Img', 4, 4, tiled=True).source
bpy.data.images.new('Img', 4, 4, alpha=True).alpha_mode
bpy.data.images.new('Img', 4)
bpy.data.images.new('Img', 4, 4, 'x')
bpy.data.meshes.new('Mesh')
bpy.data.meshes.new('').name
bpy.data.materials.new('').name
bpy.data.materials.new('Mat').node_tree.nodes.keys()
bpy.data.objects.new('CamOb', bpy.data.cameras['Cam'])
bpy.data.objects.new('SunOb', bpy.data.lights['Sun'])
bpy.data.objects.new('SpotOb', bpy.data.lights['Spot'])
bpy.data.objects.new('MeshOb', bpy.data.meshes['Mesh'])
bpy.data.objects.new('ImgOb', bpy.data.images['Img'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['SunOb'])
bpy.data.objects['SunOb'].location = (0, 0, 5)
bpy.data.cameras['Cam'].users
bpy.data.cameras.remove(bpy.data.cameras['Cam'])
bpy.data.lights.remove(bpy.data.lights['Area'])
bpy.data.worlds.remove(bpy.data.worlds['Sky'])
bpy.data.images.remove(bpy.data.images['Img.001'])
bpy.data.lights.remove(bpy.data.lights['Sun'])
