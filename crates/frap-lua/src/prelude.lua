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
local raw_load, raw_type, raw_tostring, raw_tonumber = load, type, tostring, tonumber
local raw_setmetatable, raw_getmetatable = setmetatable, getmetatable
local raw_rawget, raw_rawequal, raw_rawset, raw_rawlen, raw_select = rawget, rawequal, rawset, rawlen, select
local raw_next, raw_pairs = next, pairs
local co_resume, co_close = coroutine.resume, coroutine.close
local str_format, str_match, str_gsub, str_byte, str_find, str_sub, str_gmatch, str_rep =
    string.format, string.match, string.gsub, string.byte, string.find, string.sub, string.gmatch, string.rep
local str_pack = string.pack
local tbl_concat, tbl_sort, tbl_pack, tbl_unpack, tbl_move = table.concat, table.sort, table.pack, table.unpack, table.move
local math_type, math_tointeger, math_max, math_min = math.type, math.tointeger, math.max, math.min
local MAXINTEGER = math.maxinteger

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
        if w ~= "" and not str_find(w, "^frap%.") then
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
dofile, loadfile, require, warn, collectgarbage = nil, nil, nil, nil, nil
string.dump = nil

---------------------------------------------------------------------------- C functions
-- The deadline hook runs before every function call and every 1000 instructions, so a loop
-- stops within one call. A single C function call still runs to its end, so the ones whose
-- cost the arguments do not bound are replaced (RFC 0002 §3):
-- - `string.rep`, `table.concat`, `string.format`, `string.pack`, `load`: results over
--   `MAX_STRING` bytes are refused up front (`LIMIT_EXCEEDED`);
-- - `string.find/match/gmatch/gsub`: a call whose worst case (subject length to the power of
--   the pattern's repetitions) is over `MAX_STEPS` runs the Lua port of the C matcher
--   (`lpattern.lua`), which the hook can stop; long plain searches go in slices;
-- - `table.sort` of a long table without a comparator, `table.move` of a long range: a
--   Lua comparator and a Lua loop, which the hook can stop.

local MAX_STRING = 64 * 1024 * 1024
local MAX_STEPS = 1e7
local MAX_SORT, MAX_MOVE = 50000, 1000000
local LP = P.lpattern
local CF = {
    find = str_find,
    match = str_match,
    gmatch = str_gmatch,
    gsub = str_gsub,
    rep = str_rep,
    format = str_format,
    pack = str_pack,
    concat = tbl_concat,
    sort = tbl_sort,
    move = tbl_move,
}

-- Re-raises the error of a protected C call at the user's line, as a direct call would
-- (`lua:3: bad argument #1 to 'rep' (…)`): the C function's own name replaces the `?` it gets
-- once its library slot holds the wrapper.
local function relay(name, ok, ...)
    if ok then
        return ...
    end
    local e = (...)
    if raw_type(e) ~= "string" or e == "not enough memory" then
        raw_error(e, 0)
    end
    e = str_gsub(e, "^frap%.%w+:%d+: ", "")
    e = str_gsub(e, "^(bad argument #%d+ to )'%?'", "%1'" .. name .. "'")
    raw_error((where() or "") .. e, 0)
end

local function too_big(what, size)
    fail(
        "LIMIT_EXCEEDED",
        str_format("Error: frap: limit exceeded (%s would build %.0f bytes; at most %d)", what, size, MAX_STRING)
    )
end

-- `luaL_checklstring` (strings and numbers) and `luaL_optinteger`, or `nil` when the C
-- function would raise (the wrapper then calls it to get its own error).
local function str_arg(x)
    local t = raw_type(x)
    if t == "string" then
        return x
    elseif t == "number" then
        return raw_tostring(x)
    end
    return nil
end
local function int_arg(x, default)
    if x == nil then
        return default
    end
    local t = raw_type(x)
    if t == "number" or t == "string" then
        return math_tointeger(x)
    end
    return nil
end

-- The wrappers pass their arguments on as received (`...`), so the C function's errors
-- tell "no value" from `nil` as before.
string.rep = function(...)
    local s, n, sep = ...
    local S, N = str_arg(s), int_arg(n)
    local Sep = sep == nil and "" or str_arg(sep)
    if S and N and Sep and N > 0 then
        local size = (#S + #Sep) * (N + 0.0) - #Sep
        if size > MAX_STRING then
            too_big("string.rep", size)
        end
    end
    return relay("rep", raw_pcall(CF.rep, ...))
end

table.concat = function(...)
    local t, sep, i, j = ...
    local Sep = sep == nil and "" or str_arg(sep)
    local I, J = int_arg(i, 1), int_arg(j, false)
    if raw_type(t) == "table" and Sep and I and J ~= nil then
        if J == false then
            J = #t
        end
        local size = 0.0
        if J >= I then
            size = (J - I + 0.0) * #Sep
        end
        for k = I, J do
            local v = t[k]
            local tv = raw_type(v)
            if tv == "string" then
                size = size + #v
            elseif tv == "number" then
                size = size + 44
            else
                break -- the C function raises "invalid value (at index k)"
            end
            if size > MAX_STRING then
                too_big("table.concat", size)
            end
        end
    end
    return relay("concat", raw_pcall(CF.concat, ...))
end

string.format = function(...)
    local F = str_arg((...))
    if F then
        local args = tbl_pack(raw_select(2, ...))
        local size = #F + 0.0
        for k = 1, args.n do
            local v = args[k]
            -- `%q` writes at most 4 bytes per byte; a number at most a few hundred.
            size = size + (raw_type(v) == "string" and 4 * #v + 2 or 512)
        end
        if size > MAX_STRING then
            too_big("string.format", size)
        end
    end
    return relay("format", raw_pcall(CF.format, ...))
end

string.pack = function(...)
    local F = str_arg((...))
    if F then
        local args = tbl_pack(raw_select(2, ...))
        local size = 16.0 * #F
        for d in str_gmatch(F, "c(%d+)") do
            size = size + (raw_tonumber(d) or 0)
        end
        for k = 1, args.n do
            local v = args[k]
            if raw_type(v) == "string" then
                size = size + #v
            end
        end
        if size > MAX_STRING then
            too_big("string.pack", size)
        end
    end
    return relay("pack", raw_pcall(CF.pack, ...))
end

-- Text chunks only: binary chunks can break the VM's invariants. Compiling is one C call,
-- so the chunk's size is bounded too.
load = function(chunk, name, _mode, env)
    if raw_type(chunk) == "string" and #chunk > MAX_STRING then
        too_big("load", #chunk)
    end
    if env == nil then
        return raw_load(chunk, name, "t")
    end
    return raw_load(chunk, name, "t", env)
end

-- Worst case of the C matcher on `n` subject bytes: every start position (one when anchored)
-- times the pattern length times `n + 1` per `*`, `+`, `-`, `%b` or back-reference (each
-- can backtrack over the whole subject) and 2 per `?`. Over-counting only sends a cheap call
-- to the slower Lua matcher.
local function cheap(p, n, anchored)
    local m = #p
    local cost = (m + 1.0) * (anchored and 1 or (n + 1))
    local i = 1
    while i <= m do
        local c = str_byte(p, i)
        local item = true
        if c == 37 then -- '%'
            local d = str_byte(p, i + 1)
            if d == 98 then -- '%bxy'
                cost = cost * (n + 1)
                i = i + 4
                item = false
            elseif d == 102 then -- '%f', its set follows
                i = i + 2
                item = false
            elseif d and d >= 48 and d <= 57 then -- back-reference
                cost = cost * (n + 1)
                i = i + 2
                item = false
            else
                i = i + 2
            end
        elseif c == 91 then -- '[set]'
            i = i + 1
            if str_byte(p, i) == 94 then
                i = i + 1
            end
            repeat
                if i > m then
                    return false -- malformed: the Lua matcher raises C's error
                end
                local cc = str_byte(p, i)
                i = i + 1
                if cc == 37 then
                    i = i + 1
                end
            until str_byte(p, i) == 93
            i = i + 1
        elseif c == 40 or c == 41 then
            i = i + 1
            item = false
        else
            i = i + 1
        end
        if item then
            local q = str_byte(p, i)
            if q == 42 or q == 43 or q == 45 then
                cost = cost * (n + 1)
                i = i + 1
            elseif q == 63 then
                cost = cost * 2
                i = i + 1
            end
        end
        if cost > MAX_STEPS then
            return false
        end
    end
    return true
end

local function posrelat(pos, len)
    if pos > 0 then
        return pos
    elseif pos == 0 or pos < -len then
        return 1
    end
    return len + pos + 1
end

local SPECIALS = "[%^%$%*%+%?%.%(%[%%%-]"

-- `lmemfind` from `init`, in slices of at most `MAX_STEPS` comparisons per C call.
local function plain_find(S, Pt, init)
    local n, m = #S, #Pt
    if m == 0 or (n - init + 2.0) * (m + 1) <= MAX_STEPS then
        return CF.find(S, Pt, init, true)
    end
    local chunk = math_max(1, MAX_STEPS // (m + 1))
    local pos = init
    while pos + m - 1 <= n do
        local a = CF.find(str_sub(S, pos, math_min(n, pos + chunk + m - 2)), Pt, 1, true)
        if a then
            return a + pos - 1, a + pos + m - 2
        end
        pos = pos + chunk
    end
    return nil
end

local function find_aux(name, find, ...)
    local s, p, init, plain = ...
    local S, Pt, I = str_arg(s), str_arg(p), int_arg(init, 1)
    if not (S and Pt and I) then
        return relay(name, raw_pcall(CF[name], ...))
    end
    local n = #S
    local i = posrelat(I, n)
    if i > n + 1 then
        return nil
    end
    if find and (plain or not str_find(Pt, SPECIALS)) then
        return plain_find(S, Pt, i)
    end
    if cheap(Pt, n - i + 1, str_byte(Pt, 1) == 94) then
        return relay(name, raw_pcall(CF[name], S, Pt, i))
    end
    return relay(name, raw_pcall(LP[name], S, Pt, i))
end

string.find = function(...)
    return find_aux("find", true, ...)
end

string.match = function(...)
    return find_aux("match", false, ...)
end

string.gmatch = function(...)
    local s, p, init = ...
    local S, Pt, I = str_arg(s), str_arg(p), int_arg(init, 1)
    if S and Pt and I and not cheap(Pt, #S, false) then
        return LP.gmatch(S, Pt, I)
    end
    return relay("gmatch", raw_pcall(CF.gmatch, ...))
end

local REPL = { string = true, number = true, table = true, ["function"] = true }

string.gsub = function(...)
    local s, p, repl, n = ...
    local S, Pt, N = str_arg(s), str_arg(p), int_arg(n, false)
    if S and Pt and N ~= nil and REPL[raw_type(repl)] and not cheap(Pt, #S, str_byte(Pt, 1) == 94) then
        return relay("gsub", raw_pcall(LP.gsub, S, Pt, repl, N or nil))
    end
    return relay("gsub", raw_pcall(CF.gsub, ...))
end

local function less(a, b)
    return a < b
end

table.sort = function(...)
    local t, comp = ...
    if comp == nil and raw_type(t) == "table" and raw_getmetatable(t) == nil and raw_rawlen(t) > MAX_SORT then
        return relay("sort", raw_pcall(CF.sort, t, less))
    end
    return relay("sort", raw_pcall(CF.sort, ...))
end

table.move = function(...)
    local a1, f, e, t, a2 = ...
    local F, E, T = int_arg(f), int_arg(e), int_arg(t)
    if F and E and T and E >= F and (E + 0.0) - F >= MAX_MOVE and F > math.mininteger then
        -- The C function checks the tables (an empty range moves nothing); then its two
        -- range checks and its loop, in Lua.
        relay("move", raw_pcall(CF.move, a1, F, F - 1, T, a2))
        if not (F > 0 or E < MAXINTEGER + F) then
            raw_error((where() or "") .. "bad argument #3 to 'move' (too many elements to move)", 0)
        end
        local n = E - F + 1
        if not (T <= MAXINTEGER - n + 1) then
            raw_error((where() or "") .. "bad argument #4 to 'move' (destination wrap around)", 0)
        end
        local dst = a1
        if a2 ~= nil then
            dst = a2
        end
        if T > E or T > F or (a2 ~= nil and not (a1 == a2)) then
            for k = 0, n - 1 do
                dst[T + k] = a1[F + k]
            end
        else
            for k = n - 1, 0, -1 do
                dst[T + k] = a1[F + k]
            end
        end
        return dst
    end
    return relay("move", raw_pcall(CF.move, ...))
end

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

-- A proxy of a path inside a datablock (`bpy.data.objects['Cube'].location`) is anchored to
-- the datablock itself, not to its name: `anchor` is the datablock's handle (its `IdKey` on
-- the Rust side), `suffix` the rest of the path (`.location`), `stype` the struct type that
-- Blender's `ReferenceError` names. Every use re-reads the datablock's current path, so a
-- proxy keeps working after a rename, and raises once the datablock is removed (a new one
-- with the same name is a different datablock), as a Python reference does.
local function proxy(kind, path, owner)
    local p = {}
    local i = { kind = kind, path = path, owner = owner }
    if str_find(path, "^bpy%.data%.[%w_]+%[") then
        i.anchor, i.suffix, i.stype = P.anchor(path)
    end
    info[p] = i
    return raw_setmetatable(p, Proxy)
end

-- The path a proxy stands for now (raises `StructRNA of type X has been removed`).
local function cur(i)
    if i.anchor then
        return check(P.base(i.anchor, i.stype)) .. i.suffix
    end
    return i.path
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
        return proxy("array", path, owner)
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
        return read(attr(cur(i), k), p)
    elseif kind == "coll" then
        local path = cur(i)
        if is_ident(k) then
            local ok, v, rna, rpath = P.get(path .. "." .. k)
            if ok then
                return wrap(v, rna, rpath, p, path .. "." .. k)
            elseif v.exception ~= "AttributeError" then
                raise(v)
            end
        end
        return read(subscript(path, k), p)
    elseif kind == "array" then
        if components[k] then
            return read(cur(i) .. "." .. k)
        end
        return read(subscript(cur(i), k))
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
        write(attr(cur(i), k), v)
    elseif kind == "array" then
        if components[k] then
            write(cur(i) .. "." .. k, v)
        else
            write(subscript(cur(i), k), v)
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
            return call_path(cur(i), n - 1, raw_select(2, ...))
        end
        return call_path(cur(i), n, ...)
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
    elseif i.anchor then
        local ok, base = P.base(i.anchor, i.stype)
        if not ok then
            return "<bpy_struct, " .. i.stype .. " invalid>"
        end
        return base .. i.suffix
    end
    return i.path
end

Proxy.__eq = function(a, b)
    local ia, ib = info[a], info[b]
    if ia == nil or ib == nil or ia.kind ~= ib.kind then
        return false
    end
    if ia.anchor and ib.anchor then
        return ia.anchor == ib.anchor and ia.suffix == ib.suffix
    end
    return cur(ia) == cur(ib)
end

local function raw_value(path)
    return (check(P.get(path)))
end

Proxy.__len = function(p)
    local i = info[p]
    local v = raw_value(cur(i))
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
    return cur(i)
end

-- `rawset` cannot shadow a proxy's attributes (or `None`'s), whose data stays in `info`.
rawset = function(t, k, v)
    if info[t] ~= nil or raw_rawequal(t, None) then
        fail("LUA_VALUE", "frap: rawset cannot write into a bpy proxy")
    end
    return raw_rawset(t, k, v)
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
