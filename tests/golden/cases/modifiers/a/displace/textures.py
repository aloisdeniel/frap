# WP 4.2: DISPLACE with textures: a NONE texture reads intensity 0; an IMAGE texture on a
# generated blank image reads its stored bytes (no colour management) averaged over RGB,
# with brightness/contrast/saturation and RGB factors, and RGB_TO_XYZ in global space; an
# IMAGE texture without an image reads 0; textures.remove clears the modifier's pointer.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003 tier=T1
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.textures.new('None', type='NONE')
bpy.data.textures.new('Img', type='IMAGE')
bpy.data.textures.new('Empty', type='IMAGE')
bpy.data.textures.new('Gone', type='IMAGE')
bpy.data.images.new('Gen', 8, 4)
bpy.data.images['Gen'].generated_color = (0.8, 0.3, 0.1, 1)
bpy.data.textures['Img'].image = bpy.data.images['Gen']
bpy.data.textures['Img'].intensity = 1.2
bpy.data.textures['Img'].contrast = 1.5
bpy.data.textures['Img'].saturation = 0.5
bpy.data.textures['Img'].factor_green = 1.3
bpy.data.objects['Cube'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['Displace'].texture = bpy.data.textures['None']
bpy.data.objects['Cube'].modifiers['Displace'].direction = 'Y'
bpy.data.objects['Cube.001'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['Displace'].texture = bpy.data.textures['Img']
bpy.data.objects['Cube.001'].modifiers['Displace'].strength = 0.7
bpy.data.objects['Cube.002'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube.002'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['Displace'].texture = bpy.data.textures['Img']
bpy.data.objects['Cube.002'].modifiers['Displace'].direction = 'RGB_TO_XYZ'
bpy.data.objects['Cube.002'].modifiers['Displace'].space = 'GLOBAL'
bpy.data.objects['Cube.002'].modifiers['Displace'].texture_coords = 'UV'
bpy.data.objects['Cube.002'].modifiers.new(name='Displace.001', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['Displace.001'].texture = bpy.data.textures['Empty']
bpy.data.objects['Cube.003'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['Displace'].texture = bpy.data.textures['Gone']
bpy.data.textures.remove(bpy.data.textures['Gone'])
bpy.data.objects['Cube.003'].modifiers['Displace'].texture
