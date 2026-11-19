# SPDX-License-Identifier: GPL-3.0-or-later
# WP 3.6: Blender 5.2.4's glTF import of one material using every KHR_materials_* extension
# with a texture on every texture member, dumped for the raise tests of frap-material
# (`patterns/transmission/ext_tests.rs`) and frap-io-gltf (`material_ext.rs` tests).
#
#   blender -b --factory-startup --python dump.py -- <this directory>
#
# Writes `import_ext.gltf` (the input: 12 one-pixel PNG images as data URIs) and
# `import_ext.blender.json`: the imported material's nodes (name, bl_idname, label, location,
# parent, props) and links in `tree.nodes` / `tree.links` order, the image colour spaces, the
# unlinked socket values of the Principled BSDF, the settings group node and the Volume
# Absorption node, and the material Blender exports again (`materials[0]`).
import bpy, json, sys, base64, struct, tempfile, zlib
out = sys.argv[-1]


def png1():
    raw = b'\x00\xff\x80\x40\xff'

    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


uri = 'data:image/png;base64,' + base64.b64encode(png1()).decode()
pos = struct.pack('<9f', 0, 0, 0, 1, 0, 0, 0, 1, 0)
uv = struct.pack('<6f', 0, 0, 1, 0, 0, 1)
buf = pos + uv
gltf = {
    "asset": {"version": "2.0"},
    "scene": 0, "scenes": [{"nodes": [0]}],
    "nodes": [{"mesh": 0, "name": "N"}],
    "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "TEXCOORD_0": 1}, "material": 0}]}],
    "buffers": [{"byteLength": len(buf),
                 "uri": 'data:application/octet-stream;base64,' + base64.b64encode(buf).decode()}],
    "bufferViews": [{"buffer": 0, "byteLength": 36}, {"buffer": 0, "byteOffset": 36, "byteLength": 24}],
    "accessors": [
        {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]},
        {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC2"}],
    "images": [{"uri": uri, "name": "img%d" % i} for i in range(12)],
    "textures": [{"source": i} for i in range(12)],
    "extensionsUsed": ["KHR_materials_clearcoat", "KHR_materials_sheen", "KHR_materials_volume",
                       "KHR_materials_specular", "KHR_materials_anisotropy", "KHR_materials_iridescence",
                       "KHR_materials_transmission", "KHR_materials_ior", "KHR_materials_dispersion"],
    "materials": [{
        "name": "M",
        "extensions": {
            "KHR_materials_clearcoat": {"clearcoatFactor": 0.5, "clearcoatTexture": {"index": 0},
                                        "clearcoatRoughnessFactor": 0.25, "clearcoatRoughnessTexture": {"index": 1},
                                        "clearcoatNormalTexture": {"index": 2, "scale": 0.5}},
            "KHR_materials_transmission": {"transmissionFactor": 1.0, "transmissionTexture": {"index": 3}},
            "KHR_materials_volume": {"thicknessFactor": 2.0, "thicknessTexture": {"index": 4},
                                     "attenuationDistance": 0.5, "attenuationColor": [0.5, 0.25, 1.0]},
            "KHR_materials_dispersion": {"dispersion": 0.125},
            "KHR_materials_specular": {"specularFactor": 0.5, "specularTexture": {"index": 5},
                                       "specularColorFactor": [0.5, 1.0, 0.25], "specularColorTexture": {"index": 6}},
            "KHR_materials_anisotropy": {"anisotropyStrength": 0.5, "anisotropyRotation": 1.25,
                                         "anisotropyTexture": {"index": 7}},
            "KHR_materials_sheen": {"sheenColorFactor": [1.0, 0.5, 0.25], "sheenColorTexture": {"index": 8},
                                    "sheenRoughnessFactor": 0.75, "sheenRoughnessTexture": {"index": 9}},
            "KHR_materials_iridescence": {"iridescenceFactor": 0.5, "iridescenceTexture": {"index": 10},
                                          "iridescenceIor": 1.4, "iridescenceThicknessMinimum": 150.0,
                                          "iridescenceThicknessMaximum": 600.0,
                                          "iridescenceThicknessTexture": {"index": 11}},
            "KHR_materials_ior": {"ior": 1.25},
        }}],
}
path = out + '/import_ext.gltf'
with open(path, 'w') as f:
    json.dump(gltf, f, indent=1)
    f.write('\n')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath=path)
t = bpy.data.materials['M'].node_tree


def r(v):
    return [round(x, 4) for x in v]


def val(dv):
    try:
        return r(dv)
    except TypeError:
        return round(dv, 6) if isinstance(dv, float) else dv


nodes = []
for n in t.nodes:
    props = {}
    for p in ('operation', 'data_type', 'blend_type', 'uv_map', 'direction_type', 'interpolation', 'extension'):
        if hasattr(n, p):
            props[p] = getattr(n, p)
    if n.bl_idname == 'ShaderNodeValue':
        props['value'] = round(n.outputs[0].default_value, 6)
    if n.bl_idname == 'ShaderNodeGroup':
        props['node_tree'] = n.node_tree.name
    nodes.append([n.name, n.bl_idname, n.label, r(n.location), n.parent.name if n.parent else None, props])


def unlinked(node):
    return {s.identifier: val(s.default_value) for s in node.inputs
            if hasattr(s, 'default_value') and not s.is_linked and s.enabled}


tmp = tempfile.mkdtemp()
bpy.ops.export_scene.gltf(filepath=tmp + '/again.gltf', export_format='GLTF_SEPARATE')
again = json.load(open(tmp + '/again.gltf'))
dump = {
    "blender": bpy.app.version_string,
    "nodes": nodes,
    "links": [[l.from_node.name, l.from_socket.identifier, l.to_node.name, l.to_socket.identifier] for l in t.links],
    "images": {n.name: [n.image.name, n.image.colorspace_settings.name]
               for n in t.nodes if n.bl_idname == 'ShaderNodeTexImage'},
    "bsdf": unlinked(t.nodes['Principled BSDF']),
    "group": unlinked(t.nodes['Group']),
    "volume_absorption": unlinked(t.nodes['Volume Absorption']),
    "again": again['materials'][0],
}
with open(out + '/import_ext.blender.json', 'w') as f:
    json.dump(dump, f, indent=1)
    f.write('\n')
