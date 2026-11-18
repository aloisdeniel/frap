// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-type constants of node RNA (WP 3.1): `Node.type` (= `bl_static_type`), `color_tag`,
//! `bl_width_min`/`bl_width_max` and `bl_description` of every node type `nodes.new` accepts in
//! a shader tree, and the labels of the socket classes. `bl_label` is the type's default name
//! and `bl_width_default` its default width (generated `NodeTypeDef`; equal on every type).
//!
//! Values dumped from Blender 5.2.4 (`--factory-startup`, `_node_rna.blender.json`, written by
//! `_dump_node_rna.py`); `rna/tests.rs` checks every node and socket read against the dump.

/// Constants of one node type.
#[derive(Debug)]
pub struct NodeInfo {
    /// `bl_idname`
    pub bl_idname: &'static str,
    /// `Node.type` / `bl_static_type` ("BSDF_PRINCIPLED", "GROUP", …).
    pub ty: &'static str,
    /// `Node.color_tag`
    pub color_tag: &'static str,
    /// `bl_width_min`
    pub width_min: f32,
    /// `bl_width_max`
    pub width_max: f32,
    /// `bl_description`
    pub description: &'static str,
}

/// Every node type, sorted by `bl_idname`.
#[rustfmt::skip]
pub static NODE_INFO: [NodeInfo; 103] = [
    NodeInfo { bl_idname: "NodeFrame", ty: "FRAME", color_tag: "NONE", width_min: 100.0, width_max: f32::MAX, description: "Collect related nodes together in a common area. Useful for organization when the re-usability of a node group is not required" },
    NodeInfo { bl_idname: "NodeGroupInput", ty: "GROUP_INPUT", color_tag: "INTERFACE", width_min: 100.0, width_max: 700.0, description: "Expose connected data from inside a node group as inputs to its interface" },
    NodeInfo { bl_idname: "NodeGroupOutput", ty: "GROUP_OUTPUT", color_tag: "INTERFACE", width_min: 100.0, width_max: 700.0, description: "Output data from inside of a node group" },
    NodeInfo { bl_idname: "NodeReroute", ty: "REROUTE", color_tag: "NONE", width_min: 100.0, width_max: 700.0, description: "A single-socket organization tool that supports one input and multiple outputs" },
    NodeInfo { bl_idname: "ShaderNodeAddShader", ty: "ADD_SHADER", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Add two Shaders together" },
    NodeInfo { bl_idname: "ShaderNodeAmbientOcclusion", ty: "AMBIENT_OCCLUSION", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Compute how much the hemisphere above the shading point is occluded, for example to add weathering effects to corners.\nNote: For Cycles, this may slow down renders significantly" },
    NodeInfo { bl_idname: "ShaderNodeAttribute", ty: "ATTRIBUTE", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve attributes attached to objects or geometry" },
    NodeInfo { bl_idname: "ShaderNodeBackground", ty: "BACKGROUND", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Add background light emission.\nNote: This node should only be used for the world surface output" },
    NodeInfo { bl_idname: "ShaderNodeBevel", ty: "BEVEL", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Generates normals with round corners.\nNote: only supported in Cycles, and may slow down renders" },
    NodeInfo { bl_idname: "ShaderNodeBlackbody", ty: "BLACKBODY", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Convert a blackbody temperature to an RGB value" },
    NodeInfo { bl_idname: "ShaderNodeBrightContrast", ty: "BRIGHTCONTRAST", color_tag: "COLOR", width_min: 100.0, width_max: 700.0, description: "Control the brightness and contrast of the input color" },
    NodeInfo { bl_idname: "ShaderNodeBsdfAnisotropic", ty: "BSDF_GLOSSY", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Reflection with microfacet distribution, used for materials such as metal or mirrors" },
    NodeInfo { bl_idname: "ShaderNodeBsdfDiffuse", ty: "BSDF_DIFFUSE", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Lambertian and Oren-Nayar diffuse reflection" },
    NodeInfo { bl_idname: "ShaderNodeBsdfGlass", ty: "BSDF_GLASS", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Glass-like shader mixing refraction and reflection at grazing angles" },
    NodeInfo { bl_idname: "ShaderNodeBsdfHair", ty: "BSDF_HAIR", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Reflection and transmission shaders optimized for hair rendering" },
    NodeInfo { bl_idname: "ShaderNodeBsdfHairPrincipled", ty: "BSDF_HAIR_PRINCIPLED", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Physically-based, easy-to-use shader for rendering hair and fur" },
    NodeInfo { bl_idname: "ShaderNodeBsdfMetallic", ty: "BSDF_METALLIC", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Metallic reflection with microfacet distribution, and metallic fresnel" },
    NodeInfo { bl_idname: "ShaderNodeBsdfPrincipled", ty: "BSDF_PRINCIPLED", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Physically-based, easy-to-use shader for rendering surface materials, based on the OpenPBR model" },
    NodeInfo { bl_idname: "ShaderNodeBsdfRayPortal", ty: "BSDF_RAY_PORTAL", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Continue tracing from an arbitrary new position and in a new direction" },
    NodeInfo { bl_idname: "ShaderNodeBsdfRefraction", ty: "BSDF_REFRACTION", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Glossy refraction with sharp or microfacet distribution, typically used for materials that transmit light" },
    NodeInfo { bl_idname: "ShaderNodeBsdfSheen", ty: "BSDF_SHEEN", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Reflection for materials such as cloth.\nTypically mixed with other shaders (such as a Diffuse Shader) and is not particularly useful on its own" },
    NodeInfo { bl_idname: "ShaderNodeBsdfToon", ty: "BSDF_TOON", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Diffuse and Glossy shaders with cartoon light effects" },
    NodeInfo { bl_idname: "ShaderNodeBsdfTranslucent", ty: "BSDF_TRANSLUCENT", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Lambertian diffuse transmission" },
    NodeInfo { bl_idname: "ShaderNodeBsdfTransparent", ty: "BSDF_TRANSPARENT", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Transparency without refraction, passing straight through the surface as if there were no geometry" },
    NodeInfo { bl_idname: "ShaderNodeBump", ty: "BUMP", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Generate a perturbed normal from a height texture for bump mapping. Typically used for faking highly detailed surfaces" },
    NodeInfo { bl_idname: "ShaderNodeCameraData", ty: "CAMERA", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve information about the camera and how it relates to the current shading point's position" },
    NodeInfo { bl_idname: "ShaderNodeClamp", ty: "CLAMP", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Clamp a value between a minimum and a maximum" },
    NodeInfo { bl_idname: "ShaderNodeCombineColor", ty: "COMBINE_COLOR", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Create a color from individual components using multiple models" },
    NodeInfo { bl_idname: "ShaderNodeCombineXYZ", ty: "COMBXYZ", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Create a vector from X, Y, and Z components" },
    NodeInfo { bl_idname: "ShaderNodeDisplacement", ty: "DISPLACEMENT", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Displace the surface along the surface normal" },
    NodeInfo { bl_idname: "ShaderNodeEeveeSpecular", ty: "EEVEE_SPECULAR", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Similar to the Principled BSDF node but uses the specular workflow instead of metallic, which functions by specifying the facing (along normal) reflection color. Energy is not conserved, so the result may not be physically accurate" },
    NodeInfo { bl_idname: "ShaderNodeEmission", ty: "EMISSION", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Lambertian emission shader" },
    NodeInfo { bl_idname: "ShaderNodeFloatCurve", ty: "CURVE_FLOAT", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Map an input float to a curve and outputs a float value" },
    NodeInfo { bl_idname: "ShaderNodeFresnel", ty: "FRESNEL", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Produce a blending factor depending on the angle between the surface normal and the view direction using Fresnel equations.\nTypically used for mixing reflections at grazing angles" },
    NodeInfo { bl_idname: "ShaderNodeGamma", ty: "GAMMA", color_tag: "COLOR", width_min: 100.0, width_max: 700.0, description: "Apply a gamma correction" },
    NodeInfo { bl_idname: "ShaderNodeGroup", ty: "GROUP", color_tag: "GROUP", width_min: 60.0, width_max: 700.0, description: "" },
    NodeInfo { bl_idname: "ShaderNodeHairInfo", ty: "HAIR_INFO", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve hair curve information" },
    NodeInfo { bl_idname: "ShaderNodeHoldout", ty: "HOLDOUT", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Create a \"hole\" in the image with zero alpha transparency, which is useful for compositing.\nNote: the holdout shader can only create alpha when transparency is enabled in the film settings" },
    NodeInfo { bl_idname: "ShaderNodeHueSaturation", ty: "HUE_SAT", color_tag: "COLOR", width_min: 100.0, width_max: 700.0, description: "Apply a color transformation in the HSV color model" },
    NodeInfo { bl_idname: "ShaderNodeInvert", ty: "INVERT", color_tag: "COLOR", width_min: 100.0, width_max: 700.0, description: "Invert a color, producing a negative" },
    NodeInfo { bl_idname: "ShaderNodeLayerWeight", ty: "LAYER_WEIGHT", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Produce a blending factor depending on the angle between the surface normal and the view direction.\nTypically used for layering shaders with the Mix Shader node" },
    NodeInfo { bl_idname: "ShaderNodeLightFalloff", ty: "LIGHT_FALLOFF", color_tag: "COLOR", width_min: 100.0, width_max: 700.0, description: "Manipulate how light intensity decreases over distance. Typically used for non-physically-based effects; in reality light always falls off quadratically" },
    NodeInfo { bl_idname: "ShaderNodeLightPath", ty: "LIGHT_PATH", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve the type of incoming ray for which the shader is being executed.\nTypically used for non-physically-based tricks" },
    NodeInfo { bl_idname: "ShaderNodeMapRange", ty: "MAP_RANGE", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Remap a value from a range to a target range" },
    NodeInfo { bl_idname: "ShaderNodeMapping", ty: "MAPPING", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Transform the input vector by applying translation, rotation, and scale" },
    NodeInfo { bl_idname: "ShaderNodeMath", ty: "MATH", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Perform math operations" },
    NodeInfo { bl_idname: "ShaderNodeMix", ty: "MIX", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Mix values by a factor" },
    NodeInfo { bl_idname: "ShaderNodeMixRGB", ty: "MIX_RGB", color_tag: "COLOR", width_min: 100.0, width_max: 700.0, description: "Mix two input colors" },
    NodeInfo { bl_idname: "ShaderNodeMixShader", ty: "MIX_SHADER", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Mix two shaders together. Typically used for material layering" },
    NodeInfo { bl_idname: "ShaderNodeNewGeometry", ty: "NEW_GEOMETRY", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve geometric information about the current shading point" },
    NodeInfo { bl_idname: "ShaderNodeNormal", ty: "NORMAL", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Generate a normal vector and a dot product" },
    NodeInfo { bl_idname: "ShaderNodeNormalMap", ty: "NORMAL_MAP", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Generate a perturbed normal from an RGB normal map image. Typically used for faking highly detailed surfaces" },
    NodeInfo { bl_idname: "ShaderNodeObjectInfo", ty: "OBJECT_INFO", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve information about the object instance" },
    NodeInfo { bl_idname: "ShaderNodeOutputAOV", ty: "OUTPUT_AOV", color_tag: "OUTPUT", width_min: 100.0, width_max: 700.0, description: "Arbitrary Output Variables.\nProvide custom render passes for arbitrary shader node outputs" },
    NodeInfo { bl_idname: "ShaderNodeOutputLight", ty: "OUTPUT_LIGHT", color_tag: "OUTPUT", width_min: 100.0, width_max: 700.0, description: "Output light information to a light object" },
    NodeInfo { bl_idname: "ShaderNodeOutputLineStyle", ty: "OUTPUT_LINESTYLE", color_tag: "OUTPUT", width_min: 100.0, width_max: 700.0, description: "Control the mixing of texture information into the base color of line styles" },
    NodeInfo { bl_idname: "ShaderNodeOutputMaterial", ty: "OUTPUT_MATERIAL", color_tag: "OUTPUT", width_min: 100.0, width_max: 700.0, description: "Output surface material information for use in rendering" },
    NodeInfo { bl_idname: "ShaderNodeOutputWorld", ty: "OUTPUT_WORLD", color_tag: "OUTPUT", width_min: 100.0, width_max: 700.0, description: "Output light color information to the scene's World" },
    NodeInfo { bl_idname: "ShaderNodeParticleInfo", ty: "PARTICLE_INFO", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve the data of the particle that spawned the object instance, for example to give variation to multiple instances of an object" },
    NodeInfo { bl_idname: "ShaderNodePointInfo", ty: "POINT_INFO", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve information about points in a point cloud" },
    NodeInfo { bl_idname: "ShaderNodeRGB", ty: "RGB", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "A color picker" },
    NodeInfo { bl_idname: "ShaderNodeRGBCurve", ty: "CURVE_RGB", color_tag: "COLOR", width_min: 100.0, width_max: 700.0, description: "Apply color corrections for each color channel" },
    NodeInfo { bl_idname: "ShaderNodeRGBToBW", ty: "RGBTOBW", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Convert a color's luminance to a grayscale value" },
    NodeInfo { bl_idname: "ShaderNodeRadialTiling", ty: "ShaderNodeRadialTiling", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Transform Coordinate System for Radial Tiling" },
    NodeInfo { bl_idname: "ShaderNodeRaycast", ty: "MATERIAL_RAYCAST", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Cast rays and retrieve information from the hit point" },
    NodeInfo { bl_idname: "ShaderNodeScript", ty: "SCRIPT", color_tag: "SCRIPT", width_min: 100.0, width_max: 700.0, description: "Generate an OSL shader from a file or text data-block.\nNote: OSL shaders are not supported on all GPU backends" },
    NodeInfo { bl_idname: "ShaderNodeSeparateColor", ty: "SEPARATE_COLOR", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Split a color into its individual components using multiple models" },
    NodeInfo { bl_idname: "ShaderNodeSeparateXYZ", ty: "SEPXYZ", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Split a vector into its X, Y, and Z components" },
    NodeInfo { bl_idname: "ShaderNodeShaderToRGB", ty: "SHADERTORGB", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Convert rendering effect (such as light and shadow) to color. Typically used for non-photorealistic rendering, to apply additional effects on the output of BSDFs.\nNote: only supported in EEVEE" },
    NodeInfo { bl_idname: "ShaderNodeSqueeze", ty: "SQUEEZE", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Deprecated" },
    NodeInfo { bl_idname: "ShaderNodeSubsurfaceScattering", ty: "SUBSURFACE_SCATTERING", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Subsurface multiple scattering shader to simulate light entering the surface and bouncing internally.\nTypically used for materials such as skin, wax, marble or milk" },
    NodeInfo { bl_idname: "ShaderNodeTangent", ty: "TANGENT", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Generate a tangent direction for the Anisotropic BSDF" },
    NodeInfo { bl_idname: "ShaderNodeTexBrick", ty: "TEX_BRICK", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate a procedural texture producing bricks" },
    NodeInfo { bl_idname: "ShaderNodeTexChecker", ty: "TEX_CHECKER", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate a checkerboard texture" },
    NodeInfo { bl_idname: "ShaderNodeTexCoord", ty: "TEX_COORD", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve multiple types of texture coordinates.\nTypically used as inputs for texture nodes" },
    NodeInfo { bl_idname: "ShaderNodeTexEnvironment", ty: "TEX_ENVIRONMENT", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Sample an image file as an environment texture. Typically used to light the scene with the background node" },
    NodeInfo { bl_idname: "ShaderNodeTexGabor", ty: "TEX_GABOR", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate Gabor noise" },
    NodeInfo { bl_idname: "ShaderNodeTexGradient", ty: "TEX_GRADIENT", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate interpolated color and intensity values based on the input vector" },
    NodeInfo { bl_idname: "ShaderNodeTexIES", ty: "TEX_IES", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Match real world lights with IES files, which store the directional intensity distribution of light sources" },
    NodeInfo { bl_idname: "ShaderNodeTexImage", ty: "TEX_IMAGE", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Sample an image file as a texture" },
    NodeInfo { bl_idname: "ShaderNodeTexMagic", ty: "TEX_MAGIC", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate a psychedelic color texture" },
    NodeInfo { bl_idname: "ShaderNodeTexNoise", ty: "TEX_NOISE", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate fractal Perlin noise" },
    NodeInfo { bl_idname: "ShaderNodeTexSky", ty: "TEX_SKY", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate a procedural sky texture" },
    NodeInfo { bl_idname: "ShaderNodeTexVoronoi", ty: "TEX_VORONOI", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate Worley noise based on the distance to random points. Typically used to generate textures such as stones, water, or biological cells" },
    NodeInfo { bl_idname: "ShaderNodeTexWave", ty: "TEX_WAVE", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Generate procedural bands or rings with noise" },
    NodeInfo { bl_idname: "ShaderNodeTexWhiteNoise", ty: "TEX_WHITE_NOISE", color_tag: "TEXTURE", width_min: 100.0, width_max: 700.0, description: "Calculate a random value or color based on an input seed" },
    NodeInfo { bl_idname: "ShaderNodeUVAlongStroke", ty: "UVALONGSTROKE", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "UV coordinates that map a texture along the stroke length" },
    NodeInfo { bl_idname: "ShaderNodeUVMap", ty: "UVMAP", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve a UV map from the geometry, or the default fallback if none is specified" },
    NodeInfo { bl_idname: "ShaderNodeValToRGB", ty: "VALTORGB", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Map values to colors with the use of a gradient" },
    NodeInfo { bl_idname: "ShaderNodeValue", ty: "VALUE", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Input numerical values to other nodes in the tree" },
    NodeInfo { bl_idname: "ShaderNodeVectorCurve", ty: "CURVE_VEC", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Map input vector components with curves" },
    NodeInfo { bl_idname: "ShaderNodeVectorDisplacement", ty: "VECTOR_DISPLACEMENT", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Displace the surface along an arbitrary direction" },
    NodeInfo { bl_idname: "ShaderNodeVectorMath", ty: "VECT_MATH", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Perform vector math operation" },
    NodeInfo { bl_idname: "ShaderNodeVectorRotate", ty: "VECTOR_ROTATE", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Rotate a vector around a pivot point (center)" },
    NodeInfo { bl_idname: "ShaderNodeVectorTransform", ty: "VECT_TRANSFORM", color_tag: "VECTOR", width_min: 100.0, width_max: 700.0, description: "Convert a vector, point, or normal between world, camera, and object coordinate space" },
    NodeInfo { bl_idname: "ShaderNodeVertexColor", ty: "VERTEX_COLOR", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve a color attribute, or the default fallback if none is specified" },
    NodeInfo { bl_idname: "ShaderNodeVolumeAbsorption", ty: "VOLUME_ABSORPTION", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Absorb light as it passes through the volume" },
    NodeInfo { bl_idname: "ShaderNodeVolumeCoefficients", ty: "VOLUME_COEFFICIENTS", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Model all three physical processes in a volume, represented by their coefficients" },
    NodeInfo { bl_idname: "ShaderNodeVolumeInfo", ty: "VOLUME_INFO", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Read volume data attributes from volume grids" },
    NodeInfo { bl_idname: "ShaderNodeVolumePrincipled", ty: "PRINCIPLED_VOLUME", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Combine all volume shading components into a single easy to use node" },
    NodeInfo { bl_idname: "ShaderNodeVolumeScatter", ty: "VOLUME_SCATTER", color_tag: "SHADER", width_min: 100.0, width_max: 700.0, description: "Scatter light as it passes through the volume, often used to add fog to a scene" },
    NodeInfo { bl_idname: "ShaderNodeWavelength", ty: "WAVELENGTH", color_tag: "CONVERTER", width_min: 100.0, width_max: 700.0, description: "Convert a wavelength value to an RGB value" },
    NodeInfo { bl_idname: "ShaderNodeWireframe", ty: "WIREFRAME", color_tag: "INPUT", width_min: 100.0, width_max: 700.0, description: "Retrieve the edges of an object as it appears to Cycles.\nNote: as meshes are triangulated before being processed by Cycles, topology will always appear triangulated" },
];

/// The constants of a node type (`None` for a type that is not in the table).
pub fn node_info(bl_idname: &str) -> Option<&'static NodeInfo> {
    NODE_INFO
        .binary_search_by(|i| i.bl_idname.cmp(bl_idname))
        .ok()
        .map(|i| &NODE_INFO[i])
}

/// `NodeSocket.bl_label` and `bl_subtype_label` of a socket class (`NodeSocketFloatFactor` →
/// ("Float", "Factor"); `NodeSocketVector2D` → ("Vector", "None")); the virtual socket has empty
/// labels (verified for every class of the shader node types on 5.2.4).
pub fn socket_class_labels(bl_idname: &str) -> (&'static str, &'static str) {
    const BASES: [(&str, &str); 14] = [
        ("NodeSocketBool", "Boolean"),
        ("NodeSocketCollection", "Collection"),
        ("NodeSocketColor", "Color"),
        ("NodeSocketFloat", "Float"),
        ("NodeSocketGeometry", "Geometry"),
        ("NodeSocketImage", "Image"),
        ("NodeSocketInt", "Integer"),
        ("NodeSocketMaterial", "Material"),
        ("NodeSocketMenu", "Menu"),
        ("NodeSocketObject", "Object"),
        ("NodeSocketRotation", "Rotation"),
        ("NodeSocketShader", "Shader"),
        ("NodeSocketString", "String"),
        ("NodeSocketVector", "Vector"),
    ];
    // The longest base that prefixes the class ("NodeSocketFloat" for "NodeSocketFloatAngle").
    let Some((base, label)) = BASES
        .iter()
        .filter(|(b, _)| bl_idname.starts_with(b))
        .max_by_key(|(b, _)| b.len())
    else {
        return ("", "");
    };
    let subtype = match &bl_idname[base.len()..] {
        "Angle" => "Angle",
        "ColorTemperature" => "Color Temperature",
        "Distance" => "Distance",
        "Factor" => "Factor",
        "Pixel" => "Pixel",
        "Wavelength" => "Wavelength",
        "Direction" => "Direction",
        "Euler" => "Euler Angles",
        "Translation" => "Translation",
        "XYZ" => "XYZ",
        _ => "None",
    };
    (label, subtype)
}
