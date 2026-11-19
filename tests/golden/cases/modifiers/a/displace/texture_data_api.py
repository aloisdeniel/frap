# WP 4.2: `bpy.data.textures.new/remove` and the Texture/ImageTexture binding: `type` is
# required and checked; IMAGE refines to ImageTexture, NONE stays Texture (no `image`);
# re-typing keeps the stored values; read-only pointers; `remove(do_unlink=False)` of an unused
# texture; an empty name becomes "Texture".
# golden: startup=factory tier=T0
import bpy
bpy.data.textures.new('T')
bpy.data.textures.new('T', type='IMAGE')
bpy.data.textures.new('N', type='NONE')
bpy.data.textures.new('C', type='CLOUDS')
bpy.data.textures.new('T', type='BOGUS')
bpy.data.textures['N'].image
bpy.data.textures['T'].image
bpy.data.textures['T'].extension = 'CLIP'
bpy.data.textures['T'].type = 'NONE'
bpy.data.textures['T'].extension
bpy.data.textures['T'].type = 'IMAGE'
bpy.data.textures['T'].extension
bpy.data.textures['T'].intensity = 3
bpy.data.textures['T'].node_tree = None
bpy.data.textures['T'].color_ramp = None
bpy.data.textures['T'].color_ramp
bpy.data.textures['T'].animation_data
bpy.data.textures['T'].use_clamp = True
bpy.data.textures['T'].repeat_x = 3
bpy.data.textures['C'].intensity
bpy.data.textures['C'].extension
bpy.data.images.new('I', 4, 4)
bpy.data.textures['T'].image = bpy.data.images['I']
bpy.data.textures.remove(bpy.data.textures['C'], do_unlink=False)
bpy.data.textures.new('', 'IMAGE')
bpy.data.textures['Texture'].name
bpy.data.textures.remove(bpy.data.textures['T'])
bpy.data.textures['T']
