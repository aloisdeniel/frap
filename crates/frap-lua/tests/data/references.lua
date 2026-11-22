-- SPDX-License-Identifier: GPL-3.0-or-later
-- WP 8.10: tests/blender/references.py through frap's Lua front-end (same statements, same
-- facts, one `print` per fact). A proxy follows its datablock through a rename and raises
-- once the datablock is removed; FLOAT properties read as Lua floats.

local function fact(...)
    local t = table.pack(...)
    for i = 1, t.n do
        t[i] = tostring(t[i])
    end
    print(table.concat(t, " ", 1, t.n))
end

local function attempt(label, f)
    local ok, v = pcall(f)
    if ok then
        fact(label, "ok", v)
    else
        fact(label, "error", type(v) == "table" and v.message or v)
    end
end

-- A reference survives a rename, and so do the references reached through it.
local ob = bpy.data.objects['Cube']
local loc = ob.location
local me = ob.data
ob.name = 'Box'
ob.location.x = 2
fact("rename", ob.name, ob.location.x, tostring(ob))
loc.y = 3
fact("child", loc.y, ob.location.y, me.name, me.users)

-- The usual agent pattern: the new object, renamed, then edited through the same reference.
bpy.ops.mesh.primitive_cube_add{}
local wheel = bpy.context.object
wheel.name = 'Wheel'
wheel.scale = {2, 2, 2}
fact("context", wheel.name, wheel.scale.x, bpy.data.objects['Wheel'].scale.z)

local m = bpy.data.materials.new('Steel')
m.name = 'Iron'
m.metallic = 1
fact("material", m.name, m.metallic)

-- A removed datablock raises; a new one with the same name is another datablock.
bpy.data.objects:remove(ob)
attempt("removed", function() return ob.name end)
attempt("removed-set", function() ob.name = 'X' end)
fact("removed-repr", tostring(ob))
bpy.data.objects:remove(wheel)
bpy.ops.mesh.primitive_cube_add{}
bpy.context.object.name = 'Wheel'
attempt("readded", function() return wheel.name end)
fact("readded-new", bpy.data.objects['Wheel'].name)
local tree = bpy.data.materials['Material'].node_tree
bpy.data.materials:remove(bpy.data.materials['Material'])
attempt("embedded", function() return tree.nodes end)

-- FLOAT properties read as floats, integral or not.
local cam = bpy.data.objects['Camera']
cam.location.x = 5
fact("float", cam.location.x, math.type(cam.location.x), math.type(cam.scale[0]),
    bpy.data.cameras[0].lens, math.type(bpy.data.cameras[0].lens))
attempt("int-from-float", function() cam.pass_index = cam.location.x end)
fact("float-arith", string.format("%.17g", cam.scale.x * 4611686018427387904 * 4))
