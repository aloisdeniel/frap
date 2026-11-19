# WP 3.1: socket availability per mode seen through the first-enabled-socket name lookup: a
# write by name lands on the socket enabled for the current mode (the material comparator keys
# values by identifier) and a disabled-only name is a KeyError. Map Range (data type,
# stepped), Voronoi (dimensions, feature, Minkowski), Noise (type), Vector Rotate, Metallic
# BSDF (Fresnel), Volume Scatter (phase), Mix (vector non-uniform), White Noise, Sky, Gabor.
# golden: startup=factory tier=T0 compare=materials
import bpy
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapRange')
bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs['Steps'].default_value = 2
bpy.data.materials['Material'].node_tree.nodes['Map Range'].interpolation_type = 'STEPPED'
bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs['Steps'].default_value = 3
bpy.data.materials['Material'].node_tree.nodes['Map Range'].data_type = 'FLOAT_VECTOR'
bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs['Steps'].default_value = (4, 5, 6)
bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs['From Min'].default_value = (0.5, 0.5, 0.5)
bpy.data.materials['Material'].node_tree.nodes['Map Range'].outputs['Result']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexVoronoi')
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].inputs['W'].default_value = 1
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].voronoi_dimensions = '4D'
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].inputs['W'].default_value = 2
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].inputs['Exponent'].default_value = 3
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].distance = 'MINKOWSKI'
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].inputs['Exponent'].default_value = 4
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].feature = 'N_SPHERE_RADIUS'
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].inputs['Detail'].default_value = 5
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].outputs['Radius']
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].outputs['W']
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].feature = 'SMOOTH_F1'
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].inputs['Smoothness'].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes['Voronoi Texture'].outputs['W']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexNoise')
bpy.data.materials['Material'].node_tree.nodes['Noise Texture'].inputs['Gain'].default_value = 2
bpy.data.materials['Material'].node_tree.nodes['Noise Texture'].noise_type = 'RIDGED_MULTIFRACTAL'
bpy.data.materials['Material'].node_tree.nodes['Noise Texture'].inputs['Gain'].default_value = 3
bpy.data.materials['Material'].node_tree.nodes['Noise Texture'].inputs['Offset'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Noise Texture'].noise_dimensions = '1D'
bpy.data.materials['Material'].node_tree.nodes['Noise Texture'].inputs['Vector']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVectorRotate')
bpy.data.materials['Material'].node_tree.nodes['Vector Rotate'].inputs['Rotation']
bpy.data.materials['Material'].node_tree.nodes['Vector Rotate'].rotation_type = 'EULER_XYZ'
bpy.data.materials['Material'].node_tree.nodes['Vector Rotate'].inputs['Rotation'].default_value = (0.1, 0.2, 0.3)
bpy.data.materials['Material'].node_tree.nodes['Vector Rotate'].inputs['Angle']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeBsdfMetallic')
bpy.data.materials['Material'].node_tree.nodes['Metallic BSDF'].inputs['IOR']
bpy.data.materials['Material'].node_tree.nodes['Metallic BSDF'].fresnel_type = 'PHYSICAL_CONDUCTOR'
bpy.data.materials['Material'].node_tree.nodes['Metallic BSDF'].inputs['IOR'].default_value = (1, 2, 3)
bpy.data.materials['Material'].node_tree.nodes['Metallic BSDF'].inputs['Base Color']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVolumeScatter')
bpy.data.materials['Material'].node_tree.nodes['Volume Scatter'].phase = 'MIE'
bpy.data.materials['Material'].node_tree.nodes['Volume Scatter'].inputs['Diameter'].default_value = 7
bpy.data.materials['Material'].node_tree.nodes['Volume Scatter'].inputs['Anisotropy']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'VECTOR'
bpy.data.materials['Material'].node_tree.nodes['Mix'].factor_mode = 'NON_UNIFORM'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = (0.1, 0.2, 0.3)
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A'].default_value = (1, 1, 1)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexWhiteNoise')
bpy.data.materials['Material'].node_tree.nodes['White Noise Texture'].inputs['W']
bpy.data.materials['Material'].node_tree.nodes['White Noise Texture'].noise_dimensions = '1D'
bpy.data.materials['Material'].node_tree.nodes['White Noise Texture'].inputs['W'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexSky')
bpy.data.materials['Material'].node_tree.nodes['Sky Texture'].inputs['Vector']
bpy.data.materials['Material'].node_tree.nodes['Sky Texture'].sun_disc = False
bpy.data.materials['Material'].node_tree.nodes['Sky Texture'].inputs['Vector'].default_value = (0, 0, 1)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexGabor')
bpy.data.materials['Material'].node_tree.nodes['Gabor Texture'].gabor_type = '3D'
bpy.data.materials['Material'].node_tree.nodes['Gabor Texture'].inputs['Orientation'].default_value = (0, 1, 0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeBsdfHairPrincipled')
bpy.data.materials['Material'].node_tree.nodes['Principled Hair BSDF'].model = 'HUANG'
bpy.data.materials['Material'].node_tree.nodes['Principled Hair BSDF'].parametrization = 'MELANIN'
bpy.data.materials['Material'].node_tree.nodes['Principled Hair BSDF'].inputs['Melanin'].default_value = 0.3
bpy.data.materials['Material'].node_tree.nodes['Principled Hair BSDF'].inputs['Reflection'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Principled Hair BSDF'].inputs['Coat']
