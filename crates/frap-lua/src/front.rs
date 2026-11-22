// SPDX-License-Identifier: GPL-3.0-or-later
//! The Lua 5.4 front-end (feature `lua`): a sandboxed state whose `bpy` proxies call
//! [`Session::invoke`], [`Session::get`], [`Session::set`] and [`Session::call`] (§11.6).
//!
//! Rust side: six primitives (`invoke`, `get`, `set`, `call`, `poll`, `fail`) plus `print`
//! and `timed_out`, handed to `prelude.lua` as one table that never becomes a global, and
//! the value conversions (Lua → [`Input`], response JSON → Lua). Lua side (`prelude.lua`):
//! the proxies, the sandbox wrappers and the `frap` helpers.
//!
//! A primitive never raises: it returns `true, …` or `false, err`, where `err` is a table
//! `{exception, code, message, hint, detail}` that the prelude raises with `error`. The
//! original [`FrapError`] is kept on the Rust side under the table's `__frap_error` index, so
//! [`LuaFrontend::exec`] hands back exactly what the session answered (detail, hint and
//! all), plus where in the chunk it was raised.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use frap_rna::{ErrorCode, FrapError, Input, PyExc};
use frap_session::resolve::{self, Target};
use frap_session::{Response, Session, Status};
use indexmap::IndexMap;
use mlua::chunk::ChunkMode;
use mlua::{
    AppDataRef, Function, HookTriggers, Lua, LuaOptions, MultiValue, StdLib, Table, Value, VmState,
};
use serde_json::Value as J;

use crate::errors::{LUA_ERROR, LUA_VALUE, SYNTAX};

const PRELUDE: &str = include_str!("prelude.lua");

/// Instructions between two deadline checks of the `--stmt-timeout` hook.
const HOOK_EVERY: u32 = 1000;

/// Default `memory_limit` of [`LuaConfig`]: 256 MiB for the Lua heap.
pub const DEFAULT_MEMORY_LIMIT: usize = 256 << 20;

/// Options of a Lua front-end.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct LuaConfig {
    /// Bytes the Lua heap may hold (`0` = unlimited). Exceeding it raises `RuntimeError` code
    /// `LIMIT_EXCEEDED`. The session's own memory (meshes, images) is not counted.
    pub memory_limit: usize,
}

impl Default for LuaConfig {
    fn default() -> Self {
        LuaConfig {
            memory_limit: DEFAULT_MEMORY_LIMIT,
        }
    }
}

/// The deadline of the running chunk (outside the `RefCell`: the instruction hook may fire
/// while a primitive holds the session, e.g. inside a `__gc` metamethod).
#[derive(Default)]
struct Clock {
    deadline: Cell<Option<Instant>>,
    timed_out: Cell<bool>,
}

impl Clock {
    fn check(&self) -> bool {
        if !self.timed_out.get()
            && let Some(d) = self.deadline.get()
            && Instant::now() >= d
        {
            self.timed_out.set(true);
        }
        self.timed_out.get()
    }
}

struct Inner {
    session: Session,
    responses: Vec<Response>,
    output: Vec<String>,
    /// Errors raised during the current chunk, by `__frap_error` index.
    errors: Vec<FrapError>,
}

type Shared = Rc<RefCell<Inner>>;

/// What the conversions need to recognise the prelude's values.
struct Marks {
    none: Table,
    info: Table,
    set_meta: Table,
}

/// A Lua front-end bound to one [`Session`] (one scene, many chunks; globals persist from
/// chunk to chunk, like a REPL).
pub struct LuaFrontend {
    lua: Lua,
    inner: Shared,
    clock: Rc<Clock>,
    run: Function,
}

impl std::fmt::Debug for LuaFrontend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaFrontend").finish_non_exhaustive()
    }
}

impl LuaFrontend {
    /// A front-end over `session` with the default [`LuaConfig`].
    pub fn new(session: Session) -> Result<LuaFrontend, FrapError> {
        LuaFrontend::with_config(session, LuaConfig::default())
    }

    /// A front-end over `session`. The state loads only `base` (without `dofile`,
    /// `loadfile`, binary `load`, `collectgarbage`, `warn`), `table`, `string` (without
    /// `dump`), `math`, `utf8` and `coroutine`: no `os`, `io`, `package`/`require` (so no C
    /// module loading) and no `debug`.
    pub fn with_config(session: Session, config: LuaConfig) -> Result<LuaFrontend, FrapError> {
        let libs = StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8 | StdLib::COROUTINE;
        let lua =
            Lua::new_with(libs, LuaOptions::new().catch_rust_panics(false)).map_err(internal)?;
        if config.memory_limit > 0 {
            lua.set_memory_limit(config.memory_limit)
                .map_err(internal)?;
        }
        let inner = Rc::new(RefCell::new(Inner {
            session,
            responses: Vec::new(),
            output: Vec::new(),
            errors: Vec::new(),
        }));
        let clock = Rc::new(Clock::default());
        let prims = primitives(&lua, &inner, &clock).map_err(internal)?;
        let prelude = lua
            .load(PRELUDE)
            .set_name("=frap.prelude")
            .set_mode(ChunkMode::Text)
            .into_function()
            .map_err(internal)?;
        let run: Function = prelude.call(prims.clone()).map_err(internal)?;
        let marks = Marks {
            none: prims.get("none").map_err(internal)?,
            info: prims.get("info").map_err(internal)?,
            set_meta: prims.get("set_meta").map_err(internal)?,
        };
        lua.set_app_data(marks);
        let hook_clock = clock.clone();
        lua.set_global_hook(
            HookTriggers::new().every_nth_instruction(HOOK_EVERY),
            move |_, _| {
                if hook_clock.check() {
                    Err(mlua::Error::runtime("frap: limit exceeded (stmt_timeout)"))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .map_err(internal)?;
        Ok(LuaFrontend {
            lua,
            inner,
            clock,
            run,
        })
    }

    /// Runs one Lua chunk (the unit `--stmt-timeout` applies to). Every session call it
    /// makes is its own statement, committed or rolled back on its own; an error stops the
    /// chunk and comes back as the [`FrapError`] the session raised (`detail.lua_where`
    /// names the chunk line), a Lua error as `RuntimeError` code `LUA_ERROR`, a syntax error
    /// as `SyntaxError` code `SYNTAX`, the deadline as `RuntimeError` code `TIMEOUT`.
    pub fn exec(&mut self, src: &str) -> Result<(), FrapError> {
        self.inner.borrow_mut().errors.clear();
        let timeout = self.inner.borrow().session.options().limits.stmt_timeout;
        self.clock.timed_out.set(false);
        self.clock
            .deadline
            .set(timeout.and_then(|t| Instant::now().checked_add(t)));
        let result = self.exec_inner(src);
        self.clock.deadline.set(None);
        let timed_out = self.clock.timed_out.replace(false);
        // Whatever the chunk ended with once the deadline passed (the hook's error, a
        // re-raise by a wrapped `pcall`, a user error raised while unwinding), it timed out.
        if timed_out {
            return Err(timeout_error(timeout));
        }
        result
    }

    fn exec_inner(&mut self, src: &str) -> Result<(), FrapError> {
        let chunk = match self
            .lua
            .load(src)
            .set_name("=lua")
            .set_mode(ChunkMode::Text)
            .into_function()
        {
            Ok(f) => f,
            Err(mlua::Error::SyntaxError { message, .. }) => {
                let mut e = FrapError::new(PyExc::SyntaxError, SYNTAX, message.clone());
                if let Some(line) = lua_line(&message) {
                    e = e.with_detail("lua_line", line);
                }
                return Err(e);
            }
            Err(e) => return Err(lua_error(e)),
        };
        let out: MultiValue = match self.run.call(chunk) {
            Ok(v) => v,
            Err(e) => return Err(lua_error(e)),
        };
        let mut out = out.into_iter();
        if matches!(out.next(), Some(Value::Boolean(true))) {
            return Ok(());
        }
        Err(self.error_from(out.next().unwrap_or(Value::Nil)))
    }

    /// The error a chunk ended with: the session's own error when the prelude raised one.
    fn error_from(&self, v: Value) -> FrapError {
        match v {
            Value::Table(t) => {
                let idx: Option<usize> = t.raw_get("__frap_error").ok().flatten();
                let where_: Option<String> = t.raw_get("lua_where").ok().flatten();
                let base = idx.and_then(|i| self.inner.borrow().errors.get(i).cloned());
                let mut e = base.unwrap_or_else(|| {
                    let msg = t
                        .raw_get::<Option<String>>("message")
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "error object is a table".to_owned());
                    FrapError::new(PyExc::RuntimeError, LUA_ERROR, msg)
                });
                if let Some(w) = where_ {
                    if let Some(line) = lua_line(&w) {
                        e = e.with_detail("lua_line", line);
                    }
                    e = e.with_detail("lua_where", w);
                }
                e
            }
            Value::String(s) => {
                let msg = s.to_string_lossy();
                // `pcall` turns a LUA_ERRMEM into its message, without a position.
                if msg == "not enough memory" {
                    return lua_error(mlua::Error::MemoryError(msg));
                }
                let mut e = FrapError::new(PyExc::RuntimeError, LUA_ERROR, msg.clone());
                if let Some(line) = lua_line(&msg) {
                    e = e.with_detail("lua_line", line);
                }
                e
            }
            Value::Error(e) => lua_error(*e),
            other => FrapError::new(
                PyExc::RuntimeError,
                LUA_ERROR,
                format!("lua: error object is a {} value", other.type_name()),
            ),
        }
    }

    /// The responses of the session calls made since the last call (operators, assignments,
    /// function calls and `frap.get` reads; the reads made while walking a proxy are not
    /// listed).
    pub fn take_responses(&mut self) -> Vec<Response> {
        std::mem::take(&mut self.inner.borrow_mut().responses)
    }

    /// Lines written by `print` since the last call.
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.inner.borrow_mut().output)
    }

    /// Runs `f` with the session (read access to the scene).
    pub fn with_session<R>(&self, f: impl FnOnce(&Session) -> R) -> R {
        f(&self.inner.borrow().session)
    }

    /// Bytes the Lua heap currently holds.
    pub fn used_memory(&self) -> usize {
        self.lua.used_memory()
    }

    /// Ends the front-end, giving the session back.
    pub fn into_session(self) -> Session {
        let LuaFrontend {
            lua, inner, run, ..
        } = self;
        drop(run);
        drop(lua);
        match Rc::try_unwrap(inner) {
            Ok(cell) => cell.into_inner().session,
            Err(_) => unreachable!("invariant: the Lua state, the only other owner, is dropped"),
        }
    }
}

fn timeout_error(timeout: Option<Duration>) -> FrapError {
    let ms = timeout.map_or(0, |d| d.as_millis().min(u64::MAX as u128) as u64);
    FrapError::new(
        PyExc::RuntimeError,
        frap_session::errors::TIMEOUT,
        "Error: frap: limit exceeded (stmt_timeout)",
    )
    .with_detail("stmt_timeout_ms", ms)
    .with_hint(
        "the Lua chunk was stopped; session calls that finished before the deadline stay committed",
    )
}

fn internal(e: mlua::Error) -> FrapError {
    FrapError::new(
        PyExc::RuntimeError,
        ErrorCode::INTERNAL,
        format!("Error: frap internal error: lua: {e}"),
    )
}

fn lua_error(e: mlua::Error) -> FrapError {
    match e {
        mlua::Error::MemoryError(m) => FrapError::new(
            PyExc::RuntimeError,
            crate::errors::LIMIT_EXCEEDED,
            format!("Error: frap: limit exceeded (lua memory): {m}"),
        )
        .with_hint("the Lua heap is limited (LuaConfig::memory_limit); build smaller tables"),
        mlua::Error::CallbackError { cause, .. } => lua_error((*cause).clone()),
        mlua::Error::WithContext { cause, .. } => lua_error((*cause).clone()),
        mlua::Error::RuntimeError(m) => {
            let mut err = FrapError::new(PyExc::RuntimeError, LUA_ERROR, m.clone());
            if let Some(line) = lua_line(&m) {
                err = err.with_detail("lua_line", line);
            }
            err
        }
        other => FrapError::new(PyExc::RuntimeError, LUA_ERROR, other.to_string()),
    }
}

/// The line of a Lua position prefix (`lua:12: …` → 12).
fn lua_line(msg: &str) -> Option<u64> {
    let rest = msg.strip_prefix("lua:")?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// ------------------------------------------------------------------ primitives

fn primitives(lua: &Lua, inner: &Shared, clock: &Rc<Clock>) -> mlua::Result<Table> {
    let p = lua.create_table()?;
    p.set("none", lua.create_table()?)?;

    let s = inner.clone();
    p.set(
        "invoke",
        lua.create_function(move |lua, (idname, kw): (String, Option<Table>)| {
            let marks = marks(lua)?;
            let mut inner = borrow(&s)?;
            let kwargs = match kw.map(|t| kwargs(&inner.session, &marks, &t)).transpose() {
                Ok(k) => k.unwrap_or_default(),
                Err(e) => return failed(lua, &mut inner, e),
            };
            let r = inner.session.invoke(&idname, kwargs);
            respond(lua, &mut inner, r, |lua, r| {
                let status = lua.create_string(r.status.as_str())?;
                Ok(vec![
                    Value::String(status),
                    json_to_lua(lua, &r.to_json(), true)?,
                ])
            })
        })?,
    )?;

    let s = inner.clone();
    p.set(
        "get",
        lua.create_function(move |lua, path: String| {
            let mut inner = borrow(&s)?;
            let r = inner.session.get(&path);
            reply(lua, &mut inner, r, false, |lua, r| {
                Ok(vec![
                    r.value
                        .as_ref()
                        .map_or(Ok(Value::Nil), |v| json_to_lua(lua, v, true))?,
                    r.rna
                        .as_ref()
                        .map_or(Ok(Value::Nil), |v| json_to_lua(lua, v, true))?,
                    r.path
                        .as_deref()
                        .map_or(Ok(Value::Nil), |p| lua.create_string(p).map(Value::String))?,
                ])
            })
        })?,
    )?;

    let s = inner.clone();
    p.set(
        "set",
        lua.create_function(move |lua, (path, v): (String, Value)| {
            let marks = marks(lua)?;
            let mut inner = borrow(&s)?;
            let value = match to_input(&inner.session, &marks, &v, 0) {
                Ok(v) => v,
                Err(e) => return failed(lua, &mut inner, e),
            };
            let r = inner.session.set(&path, value);
            respond(lua, &mut inner, r, |_, _| Ok(vec![]))
        })?,
    )?;

    let s = inner.clone();
    p.set(
        "call",
        lua.create_function(
            move |lua, (path, args, kw): (String, Table, Option<Table>)| {
                let marks = marks(lua)?;
                let mut inner = borrow(&s)?;
                let n: i64 = args.raw_get::<Option<i64>>("n")?.unwrap_or(0);
                let converted = (1..=n)
                    .map(|i| {
                        let v: Value = args.raw_get(i)?;
                        Ok(to_input(&inner.session, &marks, &v, 0))
                    })
                    .collect::<mlua::Result<Vec<_>>>()?
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
                    .and_then(|pos| {
                        let kw = kw
                            .map(|t| kwargs(&inner.session, &marks, &t))
                            .transpose()?
                            .unwrap_or_default();
                        Ok((pos, kw))
                    });
                let (pos, kw) = match converted {
                    Ok(x) => x,
                    Err(e) => return failed(lua, &mut inner, e),
                };
                let r = inner.session.call(&path, pos, kw);
                respond(lua, &mut inner, r, |lua, r| {
                    Ok(vec![
                        r.value
                            .as_ref()
                            .map_or(Ok(Value::Nil), |v| json_to_lua(lua, v, true))?,
                    ])
                })
            },
        )?,
    )?;

    let s = inner.clone();
    p.set(
        "poll",
        lua.create_function(move |lua, idname: String| {
            let mut inner = borrow(&s)?;
            if !is_idname(&idname) {
                let e = FrapError::new(
                    PyExc::AttributeError,
                    frap_session::errors::RNA_NO_ATTRIBUTE,
                    format!(
                        "frap: {idname:?} is not an operator idname such as mesh.primitive_cube_add"
                    ),
                );
                return failed(lua, &mut inner, e);
            }
            // The Session API has no `poll` entry point: the statement form is the same call.
            let mut rs = inner
                .session
                .exec_source(&format!("bpy.ops.{idname}.poll()"));
            let r = rs.pop().expect("invariant: one statement, one response");
            reply(lua, &mut inner, r, false, |_, r| {
                Ok(vec![Value::Boolean(matches!(r.value, Some(J::Bool(true))))])
            })
        })?,
    )?;

    let s = inner.clone();
    p.set(
        "fail",
        lua.create_function(move |lua, (code, message): (String, String)| {
            let mut inner = borrow(&s)?;
            let (exc, code) = crate::errors::prelude_code(&code);
            let e = FrapError::new(exc, code, message);
            error_table(lua, &mut inner, e)
        })?,
    )?;

    let s = inner.clone();
    p.set(
        "print",
        lua.create_function(move |_, line: String| {
            borrow(&s)?.output.push(line);
            Ok(())
        })?,
    )?;

    let c = clock.clone();
    p.set(
        "timed_out",
        lua.create_function(move |_, ()| Ok(c.check()))?,
    )?;
    Ok(p)
}

fn borrow(s: &Shared) -> mlua::Result<std::cell::RefMut<'_, Inner>> {
    s.try_borrow_mut().map_err(|_| {
        mlua::Error::runtime("frap: the session is busy (a bpy call from a __gc metamethod?)")
    })
}

fn marks(lua: &Lua) -> mlua::Result<AppDataRef<'_, Marks>> {
    lua.app_data_ref::<Marks>()
        .ok_or_else(|| mlua::Error::runtime("frap: the prelude is not loaded"))
}

/// `true, values…` for a FINISHED or CANCELLED response, `false, err` otherwise; the
/// response is logged.
fn respond(
    lua: &Lua,
    inner: &mut Inner,
    r: Response,
    ok: impl FnOnce(&Lua, &Response) -> mlua::Result<Vec<Value>>,
) -> mlua::Result<MultiValue> {
    reply(lua, inner, r, true, ok)
}

fn reply(
    lua: &Lua,
    inner: &mut Inner,
    r: Response,
    log: bool,
    ok: impl FnOnce(&Lua, &Response) -> mlua::Result<Vec<Value>>,
) -> mlua::Result<MultiValue> {
    let out = match (&r.status, &r.error) {
        (Status::Finished | Status::Cancelled, _) => {
            let mut v = vec![Value::Boolean(true)];
            v.extend(ok(lua, &r)?);
            MultiValue::from_vec(v)
        }
        (_, Some(e)) => failed(lua, inner, e.clone())?,
        (_, None) => failed(
            lua,
            inner,
            FrapError::new(
                PyExc::RuntimeError,
                ErrorCode::INTERNAL,
                format!(
                    "Error: frap internal error: {} without an error",
                    r.status.as_str()
                ),
            ),
        )?,
    };
    if log {
        inner.responses.push(r);
    }
    Ok(out)
}

fn failed(lua: &Lua, inner: &mut Inner, e: FrapError) -> mlua::Result<MultiValue> {
    let t = error_table(lua, inner, e)?;
    Ok(MultiValue::from_vec(vec![
        Value::Boolean(false),
        Value::Table(t),
    ]))
}

/// `{exception, code, message, hint, detail, __frap_error}`.
fn error_table(lua: &Lua, inner: &mut Inner, e: FrapError) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("exception", e.exception.as_str())?;
    t.raw_set("code", e.code.0)?;
    t.raw_set("message", e.message.as_str())?;
    if let Some(h) = &e.hint {
        t.raw_set("hint", h.as_str())?;
    }
    t.raw_set(
        "detail",
        json_to_lua(lua, &J::Object(e.detail.clone()), true)?,
    )?;
    t.raw_set("__frap_error", inner.errors.len())?;
    inner.errors.push(e);
    Ok(t)
}

fn is_idname(s: &str) -> bool {
    let ident = |p: &str| {
        let mut c = p.chars();
        c.next()
            .is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
            && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    };
    matches!(s.split_once('.'), Some((a, b)) if ident(a) && ident(b))
}

// ------------------------------------------------------------------ JSON → Lua

/// A response value as Lua: objects become tables with their `$`-keys (the prelude turns
/// `$ref`, `$ptr`, `$coll`, `$func` into proxies), arrays become sequences, `null` becomes
/// `nil` at the top and `None` inside containers (a sequence cannot hold `nil`),
/// `{"$float": "nan"}` the float itself.
fn json_to_lua(lua: &Lua, v: &J, top: bool) -> mlua::Result<Value> {
    Ok(match v {
        J::Null if top => Value::Nil,
        J::Null => Value::Table(marks(lua)?.none.clone()),
        J::Bool(b) => Value::Boolean(*b),
        J::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i),
            None => Value::Number(n.as_f64().unwrap_or(f64::NAN)),
        },
        J::String(s) => Value::String(lua.create_string(s)?),
        J::Array(items) => {
            let t = lua.create_table_with_capacity(items.len(), 0)?;
            for (i, x) in items.iter().enumerate() {
                t.raw_set(i + 1, json_to_lua(lua, x, false)?)?;
            }
            Value::Table(t)
        }
        J::Object(m) => {
            if m.len() == 1
                && let Some(J::String(f)) = m.get("$float")
            {
                return Ok(Value::Number(match f.as_str() {
                    "inf" => f64::INFINITY,
                    "-inf" => f64::NEG_INFINITY,
                    _ => f64::NAN,
                }));
            }
            let t = lua.create_table_with_capacity(0, m.len())?;
            for (k, x) in m {
                t.raw_set(k.as_str(), json_to_lua(lua, x, false)?)?;
            }
            Value::Table(t)
        }
    })
}

// ------------------------------------------------------------------ Lua → Input

/// Deepest table nesting accepted as a value (a cycle or a runaway structure fails cleanly).
const MAX_DEPTH: usize = 32;

fn value_error(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::TypeError, LUA_VALUE, msg)
}

fn kwargs(
    session: &Session,
    marks: &Marks,
    t: &Table,
) -> Result<IndexMap<String, Input>, FrapError> {
    let mut out = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair.map_err(|e| value_error(format!("frap: {e}")))?;
        let Value::String(k) = k else {
            return Err(value_error(format!(
                "frap: keyword names must be strings, not {}",
                k.type_name()
            )));
        };
        let k = k
            .to_str()
            .map_err(|_| value_error("frap: keyword names must be UTF-8"))?
            .to_owned();
        out.push((k, to_input(session, marks, &v, 0)?));
    }
    // Lua tables have no order: keywords go in name order, so the same table always gives
    // the same call (and the same first-error) whatever its hash layout.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out.into_iter().collect())
}

/// A Lua value as an [`Input`]: proxies become the struct they stand for ([`Input::Ptr`]) or
/// the array they read, `None` is Python's `None`, a `Set` a set, a sequence a tuple, a table
/// with string keys a dict.
fn to_input(session: &Session, marks: &Marks, v: &Value, depth: usize) -> Result<Input, FrapError> {
    if depth > MAX_DEPTH {
        return Err(value_error(format!(
            "frap: value nested deeper than {MAX_DEPTH} tables (a cycle?)"
        )));
    }
    Ok(match v {
        Value::Nil => Input::None,
        Value::Boolean(b) => Input::Bool(*b),
        Value::Integer(i) => Input::Int(*i),
        Value::Number(f) => Input::Float(*f),
        Value::String(s) => Input::Str(
            s.to_str()
                .map_err(|_| value_error("frap: strings passed to bpy must be UTF-8"))?
                .to_owned(),
        ),
        Value::Table(t) => table_input(session, marks, t, depth)?,
        other => {
            return Err(value_error(format!(
                "frap: a Lua {} cannot be passed to bpy",
                other.type_name()
            )));
        }
    })
}

fn table_input(
    session: &Session,
    marks: &Marks,
    t: &Table,
    depth: usize,
) -> Result<Input, FrapError> {
    let lua_err = |e: mlua::Error| value_error(format!("frap: {e}"));
    if t.to_pointer() == marks.none.to_pointer() {
        return Ok(Input::None);
    }
    if let Some(info) = marks
        .info
        .raw_get::<Option<Table>>(t.clone())
        .map_err(lua_err)?
    {
        let kind: String = info.raw_get("kind").map_err(lua_err)?;
        let path: String = info.raw_get("path").map_err(lua_err)?;
        return proxy_input(session, &kind, &path);
    }
    if let Some(mt) = t.metatable() {
        if mt.to_pointer() == marks.set_meta.to_pointer() {
            let items = t
                .sequence_values::<Value>()
                .map(|v| to_input(session, marks, &v.map_err(lua_err)?, depth + 1))
                .collect::<Result<_, _>>()?;
            return Ok(Input::Set(items));
        }
        return Err(value_error(
            "frap: a table with a metatable cannot be passed to bpy",
        ));
    }
    let n = t.raw_len();
    let mut seq = Vec::with_capacity(n);
    let mut dict = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair.map_err(lua_err)?;
        match k {
            Value::Integer(i) if i >= 1 && (i as usize) <= n => {}
            Value::String(s) => {
                let k = s
                    .to_str()
                    .map_err(|_| value_error("frap: dict keys must be UTF-8"))?
                    .to_owned();
                dict.push((k, to_input(session, marks, &v, depth + 1)?));
            }
            other => {
                return Err(value_error(format!(
                    "frap: table keys must be 1..n or strings, not {}",
                    match other {
                        Value::Integer(i) => format!("integer {i}"),
                        o => o.type_name().to_owned(),
                    }
                )));
            }
        }
    }
    if !dict.is_empty() && n > 0 {
        return Err(value_error(
            "frap: a table mixing a sequence and string keys is neither a tuple nor a dict",
        ));
    }
    if !dict.is_empty() {
        dict.sort_by(|a, b| a.0.cmp(&b.0));
        return Ok(Input::Dict(dict.into_iter().collect()));
    }
    for i in 1..=n {
        let v: Value = t.raw_get(i).map_err(lua_err)?;
        seq.push(to_input(session, marks, &v, depth + 1)?);
    }
    Ok(Input::Tuple(seq))
}

/// A proxy passed as a value: a struct proxy is the struct (resolved now, like the Python
/// object the expression evaluates to), an array proxy its current value.
fn proxy_input(session: &Session, kind: &str, path: &str) -> Result<Input, FrapError> {
    match kind {
        "struct" => {
            let target = parse_path(path)?;
            let t = resolve::resolve(session.host(), &target)?;
            target_input(session, t, path)
        }
        "array" => {
            let target = parse_path(path)?;
            let t = resolve::resolve(session.host(), &target)?;
            let caps = resolve::Caps {
                max_items: usize::MAX,
                max_array: usize::MAX,
            };
            let (j, _) = resolve::encode(session.host(), &t, caps)?;
            Ok(json_input(&j))
        }
        _ => Err(value_error(format!(
            "frap: {path} ({kind}) cannot be passed as a value"
        ))),
    }
}

fn target_input(session: &Session, t: Target, path: &str) -> Result<Input, FrapError> {
    match t {
        Target::Struct(p) => Ok(Input::Ptr(p)),
        Target::None => Ok(Input::None),
        Target::Prop(p) => {
            let inner = resolve::deref(session.host(), &p)?;
            match inner {
                Target::Prop(_) => Err(value_error(format!(
                    "frap: {path} cannot be passed as a value"
                ))),
                other => target_input(session, other, path),
            }
        }
        _ => Err(value_error(format!(
            "frap: {path} cannot be passed as a value"
        ))),
    }
}

fn json_input(j: &J) -> Input {
    match j {
        J::Null => Input::None,
        J::Bool(b) => Input::Bool(*b),
        J::Number(n) => match n.as_i64() {
            Some(i) => Input::Int(i),
            None => Input::Float(n.as_f64().unwrap_or(f64::NAN)),
        },
        J::String(s) => Input::Str(s.clone()),
        J::Array(a) => Input::Tuple(a.iter().map(json_input).collect()),
        J::Object(m) => match m.get("$float").and_then(J::as_str) {
            Some("inf") => Input::Float(f64::INFINITY),
            Some("-inf") => Input::Float(f64::NEG_INFINITY),
            Some(_) => Input::Float(f64::NAN),
            None => Input::Dict(m.iter().map(|(k, v)| (k.clone(), json_input(v))).collect()),
        },
    }
}

fn parse_path(path: &str) -> Result<frap_lang::PathExpr, FrapError> {
    let mut stmts = frap_lang::parse_complete(path);
    let not_a_path = || value_error(format!("frap: {path:?} is not a path"));
    if stmts.len() != 1 {
        return Err(not_a_path());
    }
    match stmts.remove(0)? {
        frap_lang::Stmt {
            cmd: frap_lang::Command::Get { target },
            ..
        } => Ok(target),
        _ => Err(not_a_path()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_line_parses_position_prefixes() {
        assert_eq!(lua_line("lua:12: attempt to call a nil value"), Some(12));
        assert_eq!(lua_line("lua:3"), Some(3));
        assert_eq!(lua_line("frap.prelude:3: x"), None);
    }

    #[test]
    fn idnames_are_two_identifiers() {
        assert!(is_idname("mesh.primitive_cube_add"));
        assert!(!is_idname("mesh.x(); import os"));
        assert!(!is_idname("mesh"));
        assert!(!is_idname("a.b.c"));
    }
}
