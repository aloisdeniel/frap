-- SPDX-License-Identifier: GPL-3.0-or-later
-- WP 8.10: the state facts of tests/blender/m1_transcript_state.py, printed through frap's
-- Lua front-end (one `print` per fact, same order, same wording). Expects the globals
-- `imported_metallic` and `rendered` (0/1) set by m1_transcript.lua.

local function num(x)
    return string.format("%.17g", x)
end

local function fact(...)
    print(table.concat({ ... }, " "))
end

local function nums(arr)
    local out = {}
    for i, v in ipairs(frap.value(arr)) do
        out[i] = num(v)
    end
    return table.unpack(out)
end

local function sorted_names(coll)
    local names = {}
    for _, item in pairs(coll) do
        names[#names + 1] = item.name
    end
    table.sort(names)
    return names
end

fact("import", "metallic", num(imported_metallic))
fact("render", "png", rendered)
for _, name in ipairs(sorted_names(bpy.data.objects)) do
    local ob = bpy.data.objects[name]
    fact("object", name, "type", ob.type)
    fact("object", name, "data", ob.data ~= nil and ob.data.name or "None")
    fact("object", name, "location", nums(ob.location))
    fact("object", name, "rotation_euler", nums(ob.rotation_euler))
    fact("object", name, "scale", nums(ob.scale))
end
for _, name in ipairs(sorted_names(bpy.data.meshes)) do
    local me = bpy.data.meshes[name]
    fact("mesh", name, "vertices", #me.vertices, "edges", #me.edges, "polygons", #me.polygons)
    local mats = {}
    for i, m in pairs(me.materials) do
        mats[i + 1] = (m ~= nil and m ~= None) and m.name or "None"
    end
    fact("mesh", name, "materials", table.unpack(mats))
end
for _, name in ipairs(sorted_names(bpy.data.materials)) do
    local ma = bpy.data.materials[name]
    fact("material", name, "diffuse_color", nums(ma.diffuse_color))
    fact("material", name, "metallic",
        num(ma.node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value))
end
for _, name in ipairs(sorted_names(bpy.data.cameras)) do
    fact("camera", name, "lens", num(bpy.data.cameras[name].lens))
end
local sc = bpy.context.scene
fact("scene", sc.name, "camera", sc.camera ~= nil and sc.camera.name or "None")
fact("scene", sc.name, "engine", sc.render.engine)
fact("scene", sc.name, "filepath", sc.render.filepath)
fact("context", "object", bpy.context.object ~= nil and bpy.context.object.name or "None")
