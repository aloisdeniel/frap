-- SPDX-License-Identifier: GPL-3.0-or-later
-- frap-lua prelude (WP 8.10, RFC 0002): the `bpy` proxies over the Session primitives.
--
-- Loaded once per Lua state as a function of the primitive table `P` (built in Rust, see
-- `front.rs`), which never becomes a global: user code reaches the session only through
-- the proxies and the `frap` helpers defined here.
--
-- Proxies are tables whose data sits in the weak table `info` (so `rawget` on a proxy sees
-- nothing) and whose metatable is locked with `__metatable`. Every proxy stands for a
-- Blender path; indices inside paths are Blender's (0-based), plain Lua tables returned as
-- values are Lua sequences (1-based).
local P = ...

local raw_pcall, raw_xpcall, raw_error = pcall, xpcall, error
local raw_load, raw_type, raw_tostring = load, type, tostring
local raw_setmetatable, raw_getmetatable = setmetatable, getmetatable
local raw_rawget, raw_rawequal, raw_select = rawget, rawequal, select
local raw_next, raw_pairs = next, pairs
local co_resume, co_close = coroutine.resume, coroutine.close
local str_format, str_match, str_gsub, str_byte, str_find =
    string.format, string.match, string.gsub, string.byte, string.find
local tbl_concat, tbl_sort, tbl_pack, tbl_unpack = table.concat, table.sort, table.pack, table.unpack
local math_type = math.type

local info = raw_setmetatable({}, { __mode = "k" })

---------------------------------------------------------------------------- errors

local ErrMeta = {
    __metatable = "frap.error",
    __tostring = function(e)
        return raw_tostring(e.exception) .. ": " .. raw_tostring(e.message)
    end,
}

-- The position of the innermost user frame ("lua:12: "), found by asking `error` for the
-- position of increasingly outer levels (the debug library is not loaded).
local function where()
    for level = 3, 40 do
        local _, w = raw_pcall(raw_error, "", level)
        if w == nil then
            break
        end
        if w ~= "" and not str_find(w, "^frap%.prelude") then
            return w
        end
    end
    return nil
end

local function raise(e)
    if raw_getmetatable(e) == nil then
        local w = where()
        if w then
            e.lua_where = str_match(w, "^(.-):? ?$")
        end
        raw_setmetatable(e, ErrMeta)
    end
    raw_error(e, 0)
end

local function fail(code, message)
    raise(P.fail(code, message))
end

local function check(ok, ...)
    if not ok then
        raise((...))
    end
    return ...
end

---------------------------------------------------------------------------- sandbox

-- A statement that ran past `--stmt-timeout` must not be resumed by a `pcall` in user code:
-- the instruction hook raises, and every protected call re-raises once the deadline passed.
local function recheck(...)
    if P.timed_out() then
        fail("TIMEOUT", "Error: frap: limit exceeded (stmt_timeout)")
    end
    return ...
end

pcall = function(...)
    return recheck(raw_pcall(...))
end
xpcall = function(...)
    return recheck(raw_xpcall(...))
end
coroutine.resume = function(...)
    return recheck(co_resume(...))
end
coroutine.close = function(...)
    return recheck(co_close(...))
end
-- Text chunks only: binary chunks can break the VM's invariants.
load = function(chunk, name, _mode, env)
    if env == nil then
        return raw_load(chunk, name, "t")
    end
    return raw_load(chunk, name, "t", env)
end
dofile, loadfile, require, warn, collectgarbage = nil, nil, nil, nil, nil
string.dump = nil

print = function(...)
    local n = raw_select("#", ...)
    local parts = {}
    for i = 1, n do
        parts[i] = raw_tostring((raw_select(i, ...)))
    end
    P.print(tbl_concat(parts, "\t"))
end

---------------------------------------------------------------------------- values

local None = P.none
raw_setmetatable(None, {
    __metatable = "frap.None",
    __tostring = function()
        return "None"
    end,
    __newindex = function()
        fail("LUA_VALUE", "None is immutable")
    end,
})

local SetMeta
SetMeta = {
    __metatable = "frap.Set",
    __index = function(s, k)
        if raw_type(k) == "string" then
            for _, v in raw_next, s do
                if v == k then
                    return true
                end
            end
            return false
        end
        return nil
    end,
    __tostring = function(s)
        if s[1] == nil then
            return "set()"
        end
        local parts = {}
        for i, v in raw_next, s do
            parts[i] = str_format("%q", v)
        end
        tbl_sort(parts)
        return "{" .. tbl_concat(parts, ", ") .. "}"
    end,
}
P.set_meta = SetMeta

-- `Set{'VERT', 'EDGE'}`: Python's `{'VERT', 'EDGE'}` (enum flags, `type={'VERT'}`).
local function Set(items)
    local s, seen = {}, {}
    if items ~= nil then
        for _, v in raw_pairs(items) do
            if not seen[v] then
                seen[v] = true
                s[#s + 1] = v
            end
        end
    end
    tbl_sort(s, function(a, b)
        return raw_tostring(a) < raw_tostring(b)
    end)
    return raw_setmetatable(s, SetMeta)
end

---------------------------------------------------------------------------- paths

local function is_ident(k)
    return raw_type(k) == "string" and str_match(k, "^[%a_][%w_]*$") ~= nil
end

local escapes = { ["\\"] = "\\\\", ["'"] = "\\'", ["\n"] = "\\n", ["\r"] = "\\r", ["\t"] = "\\t" }

-- A Python string literal for a key (`'Principled BSDF'`).
local function pyquote(s)
    local q = str_gsub(s, "[%c\\']", function(c)
        return escapes[c] or str_format("\\x%02x", str_byte(c))
    end)
    return "'" .. q .. "'"
end

local function subscript(path, k)
    if math_type(k) == "integer" then
        return path .. "[" .. k .. "]"
    elseif raw_type(k) == "string" then
        return path .. "[" .. pyquote(k) .. "]"
    end
    fail("LUA_VALUE", str_format("frap: %s indices must be integers or strings, not %s", path, raw_type(k)))
end

local function attr(path, k)
    if is_ident(k) then
        return path .. "." .. k
    end
    return subscript(path, k)
end

---------------------------------------------------------------------------- proxies

local Proxy = { __metatable = "frap.proxy" }
P.info = info

local function proxy(kind, path, owner)
    local p = {}
    info[p] = { kind = kind, path = path, owner = owner }
    return raw_setmetatable(p, Proxy)
end

-- Turns a decoded `get`/`call` value into Lua: `$ref`/`$ptr` become struct proxies, `$coll`
-- collection proxies, `$func` function proxies, a whole array property an array proxy (so
-- `ob.location.x = 1` writes through, as in Python), an enum-flag value a `Set`.
local function wrap(v, rna, path, owner, textual)
    if raw_type(v) ~= "table" or raw_rawequal(v, None) or raw_getmetatable(v) ~= nil then
        return v
    end
    local r = raw_rawget(v, "$ref") or raw_rawget(v, "$ptr")
    if r then
        return proxy("struct", r)
    end
    r = raw_rawget(v, "$coll")
    if r then
        return proxy("coll", r)
    end
    r = raw_rawget(v, "$func")
    if r then
        return proxy("func", textual or r, owner)
    end
    if raw_rawget(v, "$array") then
        return v
    end
    if rna and path and rna.array then
        return proxy("array", path)
    end
    if rna and rna.type == "ENUM" then
        return Set(v)
    end
    for k, x in raw_next, v do
        v[k] = wrap(x)
    end
    return v
end

local function read(path, owner)
    local v, rna, rpath = check(P.get(path))
    return wrap(v, rna, rpath, owner, path)
end

local function write(path, v)
    check(P.set(path, v))
end

-- Positional arguments and keywords of a call: a trailing plain table with string keys is
-- the keyword table (`objects.new{name='Empty', object_data=None}`); its sequence part is
-- positional.
local function split_args(n, ...)
    local args = tbl_pack(...)
    local kwargs = nil
    local last = args[n]
    if n > 0 and raw_type(last) == "table" and raw_getmetatable(last) == nil then
        for k in raw_next, last do
            if raw_type(k) ~= "number" then
                kwargs = last
                break
            end
        end
    end
    if kwargs then
        local pos = {}
        for i = 1, n - 1 do
            pos[i] = args[i]
        end
        local m = n - 1
        for i = 1, #kwargs do
            m = m + 1
            pos[m] = kwargs[i]
        end
        local kw = {}
        for k, v in raw_next, kwargs do
            if raw_type(k) ~= "number" then
                kw[k] = v
            end
        end
        pos.n = m
        return pos, kw
    end
    args.n = n
    return args, nil
end

local function call_path(path, n, ...)
    local args, kw = split_args(n, ...)
    local v = check(P.call(path, args, kw))
    return wrap(v)
end

local components = { x = true, y = true, z = true, w = true, r = true, g = true, b = true, a = true }

local function op_proxy_index(i, k)
    if k == "poll" then
        return function()
            return check(P.poll(i.path))
        end
    elseif k == "idname_py" then
        return function()
            return i.path
        end
    elseif k == "idname" then
        return function()
            local ns, name = str_match(i.path, "^([^.]+)%.(.+)$")
            return string.upper(ns) .. "_OT_" .. name
        end
    end
    fail("RNA_NO_ATTRIBUTE", str_format("'bpy.ops.%s' has no attribute '%s'", i.path, raw_tostring(k)))
end

Proxy.__index = function(p, k)
    local i = info[p]
    local kind = i.kind
    if kind == "struct" then
        return read(attr(i.path, k), p)
    elseif kind == "coll" then
        if is_ident(k) then
            local ok, v, rna, rpath = P.get(i.path .. "." .. k)
            if ok then
                return wrap(v, rna, rpath, p, i.path .. "." .. k)
            elseif v.exception ~= "AttributeError" then
                raise(v)
            end
        end
        return read(subscript(i.path, k), p)
    elseif kind == "array" then
        if components[k] then
            return read(i.path .. "." .. k)
        end
        return read(subscript(i.path, k))
    elseif kind == "ops" then
        if not is_ident(k) then
            fail("RNA_NO_ATTRIBUTE", str_format("module 'bpy.ops' has no attribute %q", raw_tostring(k)))
        end
        return proxy("opns", k)
    elseif kind == "opns" then
        if not is_ident(k) then
            fail("RNA_NO_ATTRIBUTE", str_format("'bpy.ops.%s' has no attribute %q", i.path, raw_tostring(k)))
        end
        return proxy("op", i.path .. "." .. k)
    elseif kind == "op" then
        return op_proxy_index(i, k)
    end
    fail("RNA_NO_ATTRIBUTE", str_format("'%s' has no attribute %q", i.path, raw_tostring(k)))
end

Proxy.__newindex = function(p, k, v)
    local i = info[p]
    local kind = i.kind
    if kind == "struct" or kind == "coll" then
        write(attr(i.path, k), v)
    elseif kind == "array" then
        if components[k] then
            write(i.path .. "." .. k, v)
        else
            write(subscript(i.path, k), v)
        end
    else
        fail("LUA_VALUE", str_format("frap: cannot assign to %s", i.path))
    end
end

Proxy.__call = function(p, ...)
    local i = info[p]
    local n = raw_select("#", ...)
    if i.kind == "op" then
        if n > 1 or (n == 1 and (raw_type((...)) ~= "table" or raw_getmetatable((...)) ~= nil)) then
            fail(
                "OP_ARG_POSITIONAL",
                str_format(
                    "frap: operators take keyword arguments only, as one table: bpy.ops.%s{...}",
                    i.path
                )
            )
        end
        local kw = (...)
        if kw ~= nil then
            for k in raw_next, kw do
                if raw_type(k) ~= "string" then
                    fail(
                        "OP_ARG_POSITIONAL",
                        str_format(
                            "frap: operators take keyword arguments only, as one table: bpy.ops.%s{...}",
                            i.path
                        )
                    )
                end
            end
        end
        local status, resp = check(P.invoke(i.path, kw))
        return status, resp
    elseif i.kind == "func" then
        -- `ob:select_set(true)` passes the owner first; `ob.select_set(true)` does not.
        if n > 0 and i.owner ~= nil and raw_rawequal((...), i.owner) then
            return call_path(i.path, n - 1, raw_select(2, ...))
        end
        return call_path(i.path, n, ...)
    end
    fail("RNA_TYPE", str_format("'%s' object is not callable", i.path))
end

Proxy.__tostring = function(p)
    local i = info[p]
    if i.kind == "ops" then
        return "<module like class 'bpy.ops'>"
    elseif i.kind == "opns" then
        return "<module like class 'bpy.ops." .. i.path .. "'>"
    elseif i.kind == "op" then
        return "bpy.ops." .. i.path
    end
    return i.path
end

Proxy.__eq = function(a, b)
    local ia, ib = info[a], info[b]
    return ia ~= nil and ib ~= nil and ia.kind == ib.kind and ia.path == ib.path
end

local function raw_value(path)
    return (check(P.get(path)))
end

Proxy.__len = function(p)
    local i = info[p]
    local v = raw_value(i.path)
    if raw_type(v) == "table" and raw_rawget(v, "$coll") then
        return raw_rawget(v, "len")
    elseif raw_type(v) == "table" then
        return #v
    elseif raw_type(v) == "string" then
        return #v
    end
    fail("RNA_TYPE", str_format("object of type '%s' has no len()", raw_type(v)))
end

-- `for i, ob in pairs(bpy.data.objects)`: Blender indices (0-based) and items.
local function iter(c)
    local n = #c
    local idx = -1
    return function()
        idx = idx + 1
        if idx < n then
            local v = c[idx]
            if v == nil then
                v = None
            end
            return idx, v
        end
        return nil
    end
end
Proxy.__pairs = function(p)
    return iter(p), p, nil
end

---------------------------------------------------------------------------- globals

local ops = proxy("ops", "bpy.ops")
local data = proxy("struct", "bpy.data")
local context = proxy("struct", "bpy.context")

bpy = { ops = ops, data = data, context = context }
D, C = data, context
_ENV.None = None
_ENV.Set = Set

local function path_of(x)
    local i = info[x]
    if i == nil then
        fail("LUA_VALUE", "frap.path: not a bpy proxy")
    end
    return i.path
end

frap = {
    -- `frap.get('bpy.data.objects["Cube"].location')`: a path read (proxies for structs).
    get = function(path)
        return read(path)
    end,
    -- `frap.set(path, value)`: a path assignment.
    set = function(path, v)
        write(path, v)
    end,
    -- `frap.call(path, args...)`: an RNA function call by path.
    call = function(path, ...)
        return call_path(path, raw_select("#", ...), ...)
    end,
    -- `frap.invoke('mesh.primitive_cube_add', {size=2})`: an operator by idname.
    invoke = function(idname, kw)
        return check(P.invoke(idname, kw))
    end,
    -- The Blender path a proxy stands for.
    path = path_of,
    -- The plain value of a proxy or path (arrays as Lua sequences, structs as `{["$ref"]=…}`).
    value = function(x)
        if raw_type(x) == "string" then
            return raw_value(x)
        end
        return raw_value(path_of(x))
    end,
    iter = iter,
    None = None,
    Set = Set,
    -- `error(frap.error('ValueError', 'bad'))` style errors from user code.
    is_error = function(e)
        return raw_getmetatable(e) == "frap.error"
    end,
}

-- Runs a user chunk; the error object comes back to Rust unchanged.
return function(fn)
    return raw_pcall(fn)
end
