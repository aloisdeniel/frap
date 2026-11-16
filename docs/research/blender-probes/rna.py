import bpy
A=bpy.types.Action.bl_rna.properties.keys()
print("Action props:", [k for k in A if k in ('fcurves','layers','slots','groups','frame_range','use_frame_range')])
M=bpy.types.Material.bl_rna.properties
print("Material:", [k for k in ('use_nodes','blend_method','surface_render_method','use_backface_culling','alpha_threshold','use_transparency_overlap') if k in M.keys()])
print("surface_render_method items", [i.identifier for i in M['surface_render_method'].enum_items])
L=bpy.types.Light.bl_rna.properties.keys()
print("Light:", [k for k in ('energy','exposure','normalize','use_temperature','temperature','use_custom_distance','cutoff_distance','shadow_soft_size') if k in L])
pb = bpy.types.ShaderNodeBsdfPrincipled
m = bpy.data.materials.new("x"); 
try: m.use_nodes=True
except Exception as e: print("use_nodes err", e)
n = m.node_tree.nodes['Principled BSDF']
print("Principled inputs:", [(i.name, i.identifier, i.type, (tuple(i.default_value) if hasattr(i.default_value,'__len__') else i.default_value) if hasattr(i,'default_value') else None) for i in n.inputs])
print("Mesh color attr API:", 'color_attributes' in bpy.types.Mesh.bl_rna.properties.keys(), 'vertex_colors' in bpy.types.Mesh.bl_rna.properties.keys())
print("Camera:", [k for k in ('angle','angle_x','angle_y','sensor_fit','lens_unit','ortho_scale','clip_start','clip_end') if k in bpy.types.Camera.bl_rna.properties.keys()])
