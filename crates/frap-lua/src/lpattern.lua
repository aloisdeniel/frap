-- SPDX-License-Identifier: GPL-3.0-or-later
-- frap-lua (WP 8.10, RFC 0002 §3): Lua pattern matching written in Lua, so that the
-- `--stmt-timeout` hook can stop it.
--
-- Ported from lua-5.4.9/src/lstrlib.c (the "PATTERN MATCHING" section: `match`,
-- `singlematch`, `matchbalance`, `max_expand`, `min_expand`, captures, `str_find_aux`,
-- `gmatch`, `str_gsub`, `add_s`, `add_value`), MIT licence, Copyright (C) 1994-2025 Lua.org,
-- PUC-Rio. The C functions run uninterrupted, and a backtracking pattern can take minutes
-- (`string.find(string.rep('a', 3000), 'a-a-a-b')`); the prelude sends calls whose worst case
-- is too large to this port, which runs in the VM and so stops at the deadline. Same results
-- and the same error messages as the C functions; indices are 1-based (`s == n + 1` is the C
-- `src_end`, `p == m + 1` the C `p_end`).
--
-- The callers (the prelude's wrappers) have already checked and converted the arguments:
-- `s` and `p` are strings, `init` an integer, `repl` a string, number, table or function.

local str_byte, str_sub, str_find, str_format, str_char =
    string.byte, string.sub, string.find, string.format, string.char
local tbl_concat, tbl_unpack = table.concat, table.unpack
local raw_error, raw_type, raw_tostring, raw_pcall = error, type, tostring, pcall

local CAP_UNFINISHED, CAP_POSITION = -1, -2
local MAXCCALLS = 200
local LUA_MAXCAPTURES = 32
local L_ESC = 37 -- '%'

-- The C library's character classes (C locale), read from the C matcher itself.
local CLASS = {}
for _, cl in ipairs({ "a", "c", "d", "g", "l", "p", "s", "u", "w", "x", "z" }) do
    local t = {}
    for c = 0, 255 do
        t[c] = str_find(str_char(c), "^%" .. cl) ~= nil
    end
    CLASS[str_byte(cl)] = t
end

local function err(msg)
    raw_error(msg, 0)
end

local function isdigit(c)
    return c ~= nil and c >= 48 and c <= 57
end

-- `match_class(c, cl)`: `%a`, `%A`, …, or the literal `cl`.
local function match_class(c, cl)
    if cl == nil then
        return false
    end
    local lower = cl
    if cl >= 65 and cl <= 90 then
        lower = cl + 32
    end
    local t = CLASS[lower]
    if t == nil then
        return cl == c
    end
    if cl == lower then
        return t[c]
    end
    return not t[c]
end

-- A matcher over subject `S` and pattern `P`: the C `MatchState` as closures.
local function matcher(S, P)
    local N, M = #S, #P
    local ms = { level = 0, depth = MAXCCALLS, init = {}, len = {} }
    local cinit, clen = ms.init, ms.len
    local match

    local function classend(p)
        local c = str_byte(P, p)
        p = p + 1
        if c == L_ESC then
            if p > M then
                err("malformed pattern (ends with '%')")
            end
            return p + 1
        elseif c == 91 then -- '['
            if str_byte(P, p) == 94 then
                p = p + 1
            end
            repeat
                if p > M then
                    err("malformed pattern (missing ']')")
                end
                local cc = str_byte(P, p)
                p = p + 1
                if cc == L_ESC and p <= M then
                    p = p + 1
                end
            until str_byte(P, p) == 93
            return p + 1
        end
        return p
    end

    -- `p` is the '[' and `ec` the ']' of the set.
    local function matchbracketclass(c, p, ec)
        local sig = true
        if str_byte(P, p + 1) == 94 then
            sig = false
            p = p + 1
        end
        p = p + 1
        while p < ec do
            local pc = str_byte(P, p)
            if pc == L_ESC then
                p = p + 1
                if match_class(c, str_byte(P, p)) then
                    return sig
                end
            elseif str_byte(P, p + 1) == 45 and p + 2 < ec then
                p = p + 2
                if str_byte(P, p - 2) <= c and c <= str_byte(P, p) then
                    return sig
                end
            elseif pc == c then
                return sig
            end
            p = p + 1
        end
        return not sig
    end

    local function singlematch(s, p, ep)
        if s > N then
            return false
        end
        local c = str_byte(S, s)
        local pc = str_byte(P, p)
        if pc == 46 then -- '.'
            return true
        elseif pc == L_ESC then
            return match_class(c, str_byte(P, p + 1))
        elseif pc == 91 then
            return matchbracketclass(c, p, ep - 1)
        end
        return pc == c
    end

    local function matchbalance(s, p)
        if p >= M then
            err("malformed pattern (missing arguments to '%b')")
        end
        if s > N or str_byte(S, s) ~= str_byte(P, p) then
            return nil
        end
        local b, e = str_byte(P, p), str_byte(P, p + 1)
        local cont = 1
        s = s + 1
        while s <= N do
            local c = str_byte(S, s)
            if c == e then
                cont = cont - 1
                if cont == 0 then
                    return s + 1
                end
            elseif c == b then
                cont = cont + 1
            end
            s = s + 1
        end
        return nil
    end

    local function max_expand(s, p, ep)
        local i = 0
        while singlematch(s + i, p, ep) do
            i = i + 1
        end
        while i >= 0 do
            local res = match(s + i, ep + 1)
            if res then
                return res
            end
            i = i - 1
        end
        return nil
    end

    local function min_expand(s, p, ep)
        while true do
            local res = match(s, ep + 1)
            if res then
                return res
            elseif singlematch(s, p, ep) then
                s = s + 1
            else
                return nil
            end
        end
    end

    local function start_capture(s, p, what)
        local level = ms.level
        if level >= LUA_MAXCAPTURES then
            err("too many captures")
        end
        cinit[level] = s
        clen[level] = what
        ms.level = level + 1
        local res = match(s, p)
        if res == nil then
            ms.level = ms.level - 1
        end
        return res
    end

    local function capture_to_close()
        for level = ms.level - 1, 0, -1 do
            if clen[level] == CAP_UNFINISHED then
                return level
            end
        end
        err("invalid pattern capture")
    end

    local function end_capture(s, p)
        local l = capture_to_close()
        clen[l] = s - cinit[l]
        local res = match(s, p)
        if res == nil then
            clen[l] = CAP_UNFINISHED
        end
        return res
    end

    local function check_capture(l)
        l = l - 49 -- '1'
        if l < 0 or l >= ms.level or clen[l] == CAP_UNFINISHED then
            err(str_format("invalid capture index %%%d", l + 1))
        end
        return l
    end

    local function match_capture(s, l)
        l = check_capture(l)
        local len = clen[l]
        -- A position capture has a negative length: C's `(size_t)len` never fits.
        if len >= 0 and N - s + 1 >= len and str_sub(S, cinit[l], cinit[l] + len - 1) == str_sub(S, s, s + len - 1) then
            return s + len
        end
        return nil
    end

    match = function(s, p)
        if ms.depth == 0 then
            err("pattern too complex")
        end
        ms.depth = ms.depth - 1
        while p <= M do
            local pc = str_byte(P, p)
            if pc == 40 then -- '('
                if str_byte(P, p + 1) == 41 then
                    s = start_capture(s, p + 2, CAP_POSITION)
                else
                    s = start_capture(s, p + 1, CAP_UNFINISHED)
                end
                break
            elseif pc == 41 then -- ')'
                s = end_capture(s, p + 1)
                break
            elseif pc == 36 and p == M then -- '$' at the end
                if s ~= N + 1 then
                    s = nil
                end
                break
            elseif pc == L_ESC and str_byte(P, p + 1) == 98 then -- '%b'
                s = matchbalance(s, p + 2)
                if s == nil then
                    break
                end
                p = p + 4
            elseif pc == L_ESC and str_byte(P, p + 1) == 102 then -- '%f'
                p = p + 2
                if str_byte(P, p) ~= 91 then
                    err("missing '[' after '%f' in pattern")
                end
                local ep = classend(p)
                local previous = 0
                if s ~= 1 then
                    previous = str_byte(S, s - 1)
                end
                local current = str_byte(S, s) or 0
                if not matchbracketclass(previous, p, ep - 1) and matchbracketclass(current, p, ep - 1) then
                    p = ep
                else
                    s = nil
                    break
                end
            elseif pc == L_ESC and isdigit(str_byte(P, p + 1)) then -- '%0'-'%9'
                s = match_capture(s, str_byte(P, p + 1))
                if s == nil then
                    break
                end
                p = p + 2
            else
                local ep = classend(p)
                local epc = str_byte(P, ep)
                if not singlematch(s, p, ep) then
                    if epc == 42 or epc == 63 or epc == 45 then -- '*' '?' '-'
                        p = ep + 1
                    else
                        s = nil
                        break
                    end
                elseif epc == 63 then -- '?'
                    local res = match(s + 1, ep + 1)
                    if res then
                        s = res
                        break
                    end
                    p = ep + 1
                elseif epc == 43 then -- '+'
                    s = max_expand(s + 1, p, ep)
                    break
                elseif epc == 42 then -- '*'
                    s = max_expand(s, p, ep)
                    break
                elseif epc == 45 then -- '-'
                    s = min_expand(s, p, ep)
                    break
                else
                    s = s + 1
                    p = ep
                end
            end
        end
        ms.depth = ms.depth + 1
        return s
    end

    local function reprep()
        ms.depth = MAXCCALLS
        ms.level = 0
    end

    -- Capture `i` (0-based) of the match `s`..`e - 1`: a string or a position.
    local function onecapture(i, s, e)
        if i >= ms.level then
            if i ~= 0 then
                err(str_format("invalid capture index %%%d", i + 1))
            end
            return str_sub(S, s, e - 1)
        end
        local l = clen[i]
        if l == CAP_UNFINISHED then
            err("unfinished capture")
        elseif l == CAP_POSITION then
            return cinit[i]
        end
        return str_sub(S, cinit[i], cinit[i] + l - 1)
    end

    -- The captures as a sequence (`n` = count); `s == nil`: none for a capture-less match.
    local function captures(s, e)
        local n = ms.level
        if n == 0 and s then
            n = 1
        end
        local out = { n = n }
        for i = 0, n - 1 do
            out[i + 1] = onecapture(i, s, e)
        end
        return out
    end

    return {
        N = N,
        match = match,
        reprep = reprep,
        onecapture = onecapture,
        captures = captures,
    }
end

-- C `posrelatI`.
local function posrelat(pos, len)
    if pos > 0 then
        return pos
    elseif pos == 0 then
        return 1
    elseif pos < -len then
        return 1
    end
    return len + pos + 1
end

local M = {}

-- `str_find_aux`. A plain search (no special characters) is C's `lmemfind`, which the
-- prelude runs in slices before it gets here.
local function find_aux(find, s, p, init)
    local ls = #s
    init = posrelat(init, ls)
    if init > ls + 1 then
        return nil
    end
    if find and not str_find(p, "[%^%$%*%+%?%.%(%[%%%-]") then
        return str_find(s, p, init, true)
    end
    local pstart = 1
    local anchor = str_byte(p, 1) == 94
    if anchor then
        p = str_sub(p, 2)
    end
    local m = matcher(s, p)
    local s1 = init
    while true do
        m.reprep()
        local res = m.match(s1, pstart)
        if res then
            if find then
                local c = m.captures(nil, 0)
                return s1, res - 1, tbl_unpack(c, 1, c.n)
            end
            local c = m.captures(s1, res)
            return tbl_unpack(c, 1, c.n)
        end
        if anchor or s1 > ls then
            break
        end
        s1 = s1 + 1
    end
    return nil
end

function M.find(s, p, init)
    return find_aux(true, s, p, init)
end

function M.match(s, p, init)
    return find_aux(false, s, p, init)
end

function M.gmatch(s, p, init)
    local ls = #s
    init = posrelat(init, ls)
    if init > ls + 1 then
        init = ls + 2
    end
    local m = matcher(s, p)
    local src, lastmatch = init, nil
    local function step()
        for src0 = src, ls + 1 do
            m.reprep()
            local e = m.match(src0, 1)
            if e and e ~= lastmatch then
                src, lastmatch = e, e
                return m.captures(src0, e)
            end
        end
        return nil
    end
    -- The C iterator's errors carry the position of the loop that calls it.
    return function()
        local ok, c = raw_pcall(step)
        if not ok then
            raw_error(c, 2)
        elseif c then
            return tbl_unpack(c, 1, c.n)
        end
    end
end

local function typename(v)
    return raw_type(v)
end

function M.gsub(s, p, repl, max_s)
    local srcl = #s
    local tr = raw_type(repl)
    if max_s == nil then
        max_s = srcl + 1
    end
    local anchor = str_byte(p, 1) == 94
    if anchor then
        p = str_sub(p, 2)
    end
    local m = matcher(s, p)
    local news = tr == "number" and raw_tostring(repl) or repl
    local out, nout = {}, 0
    local function add(x)
        nout = nout + 1
        out[nout] = x
    end

    -- `add_s`: the replacement string with `%0`-`%9` and `%%`.
    local function add_s(a, e)
        local i, l = 1, #news
        while true do
            local q = str_find(news, "%", i, true)
            if q == nil then
                break
            end
            add(str_sub(news, i, q - 1))
            local c = str_byte(news, q + 1)
            if c == L_ESC then
                add("%")
            elseif c == 48 then
                add(str_sub(s, a, e - 1))
            elseif isdigit(c) then
                add(raw_tostring(m.onecapture(c - 49, a, e)))
            else
                err("invalid use of '%' in replacement string")
            end
            i = q + 2
        end
        if i <= l then
            add(str_sub(news, i))
        end
    end

    local function add_value(a, e)
        local v
        if tr == "function" then
            local c = m.captures(a, e)
            v = repl(tbl_unpack(c, 1, c.n))
        elseif tr == "table" then
            v = repl[m.onecapture(0, a, e)]
        else
            add_s(a, e)
            return true
        end
        if not v then
            add(str_sub(s, a, e - 1))
            return false
        end
        local tv = raw_type(v)
        if tv ~= "string" and tv ~= "number" then
            err(str_format("invalid replacement value (a %s)", typename(v)))
        end
        add(raw_tostring(v))
        return true
    end

    local n, changed, lastmatch = 0, false, nil
    local src, copied = 1, 1 -- `copied`: the first subject byte not yet in `out`
    while n < max_s do
        m.reprep()
        local e = m.match(src, 1)
        if e and e ~= lastmatch then
            n = n + 1
            if copied < src then
                add(str_sub(s, copied, src - 1))
            end
            if add_value(src, e) then
                changed = true
            end
            src, lastmatch, copied = e, e, e
        elseif src <= srcl then
            src = src + 1
        else
            break
        end
        if anchor then
            break
        end
    end
    if not changed then
        return s, n
    end
    add(str_sub(s, copied))
    return tbl_concat(out), n
end

return M
