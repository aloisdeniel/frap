-- SPDX-License-Identifier: GPL-3.0-or-later
-- Unit check of lpattern.lua (front.rs tests): random patterns and subjects through the C
-- string functions and the Lua port, which must give the same values or the same error.
-- Returns "" or a description of the first difference. `LP` is the port.
local seed = 12345
local function rand(n)
    seed = (seed * 1103515245 + 12345) % 2147483648
    return seed % n + 1
end

local pieces = {
    "a", "b", "c", ".", "%a", "%d", "%s", "%W", "[ab]", "[^a]", "[a-c]", "[%a-]", "[]", "(", ")", "()",
    "%b()", "%f[%a]", "%f[%z]", "%1", "%2", "^", "$", "*", "+", "-", "?", "%", "[", "]", "%%", "x", " ",
}
local chars = { "a", "b", "c", "(", ")", "1", " ", "x", "-", "%" }

local function gen_pattern()
    local t = {}
    for i = 1, rand(6) do
        t[i] = pieces[rand(#pieces)]
    end
    return table.concat(t)
end

local function gen_subject()
    local t = {}
    for i = 1, rand(13) - 1 do
        t[i] = chars[rand(#chars)]
    end
    return table.concat(t)
end

local function show(ok, ...)
    local t = table.pack(...)
    local out = { tostring(ok) }
    for i = 1, t.n do
        out[#out + 1] = type(t[i]) .. ":" .. tostring(t[i])
    end
    return table.concat(out, "|")
end

local function gm(f, s, p, init)
    local it = f(s, p, init)
    local out = {}
    for _ = 1, 40 do
        local r = table.pack(it())
        if r[1] == nil then
            break
        end
        for i = 1, r.n do
            out[#out + 1] = type(r[i]) .. ":" .. tostring(r[i])
        end
        out[#out + 1] = ";"
    end
    return table.concat(out, ",")
end

local repls = {
    "<%0>", "%1", "%%", "[%1|%2]", "x", 7, "%",
    { a = "A", b = false, ["("] = 1 },
    function(a, b)
        if a == "b" then
            return nil
        end
        return (a or "") .. "/" .. tostring(b)
    end,
}

for case = 1, 30000 do
    local s, p = gen_subject(), gen_pattern()
    local init = rand(9) - 4
    local checks = {
        { "find", show(pcall(string.find, s, p, init)), show(pcall(LP.find, s, p, init)) },
        { "match", show(pcall(string.match, s, p, init)), show(pcall(LP.match, s, p, init)) },
        { "gmatch", show(pcall(gm, string.gmatch, s, p, init)), show(pcall(gm, LP.gmatch, s, p, init)) },
    }
    local r = repls[rand(#repls)]
    local n = rand(4) == 1 and rand(3) - 1 or nil
    checks[4] = { "gsub", show(pcall(string.gsub, s, p, r, n)), show(pcall(LP.gsub, s, p, r, n)) }
    for _, c in ipairs(checks) do
        if c[2] ~= c[3] then
            return string.format("case %d: %s(%q, %q, %s) C %s, Lua %s", case, c[1], s, p, tostring(init), c[2], c[3])
        end
    end
end
return ""
