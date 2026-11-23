/* OpenMT2001's protected C boundary for the third-party Lua 4.0 VM.
 * No Lua API or longjmp is entered from a Rust callback. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "lua.h"
#include "ldo.h"
#include "ltable.h"

typedef struct {
    int kind;
    double number;
    const unsigned char *bytes;
    size_t length;
} mt4_value;
typedef int (*mt4_handler)(void *, uint32_t, const mt4_value *, size_t,
                          double *, char *, size_t);
typedef struct {
    lua_State *state;
    mt4_handler handler;
    void *context;
    int scripts;
    char error[512];
} mt4_runtime;
typedef struct {
    mt4_runtime *runtime;
    int action;
    const char *name;
    const char *source;
    size_t length;
    const double *arguments;
    size_t count;
    uint32_t id;
    double number;
    int status;
    int present;
} mt4_operation;

static void copy_error(mt4_runtime *runtime, const char *text, size_t length) {
    if (length >= sizeof(runtime->error)) length = sizeof(runtime->error) - 1;
    memcpy(runtime->error, text, length);
    runtime->error[length] = 0;
}

static int capture_error(lua_State *L) {
    mt4_runtime *runtime = (mt4_runtime *)lua_touserdata(L, -1);
    if (lua_type(L, 1) == LUA_TSTRING)
        copy_error(runtime, lua_tostring(L, 1), lua_strlen(L, 1));
    return 0;
}

static int host_call(lua_State *L) {
    int count = lua_gettop(L) - 2;
    mt4_runtime *runtime = (mt4_runtime *)lua_touserdata(L, -2);
    uint32_t id = (uint32_t)lua_tonumber(L, -1);
    mt4_value values[256];
    double result = 0;
    int i, status;
    if (!runtime->handler) lua_error(L, "host callback outside active operation");
    if (count < 0 || count > 256) lua_error(L, "host callback argument limit");
    for (i = 0; i < count; ++i) {
        memset(&values[i], 0, sizeof(values[i]));
        values[i].kind = lua_type(L, i + 1);
        if (values[i].kind == LUA_TNUMBER)
            values[i].number = lua_tonumber(L, i + 1);
        else if (values[i].kind == LUA_TSTRING) {
            values[i].bytes = (const unsigned char *)lua_tostring(L, i + 1);
            values[i].length = lua_strlen(L, i + 1);
        }
        else if (values[i].kind != LUA_TNIL)
            lua_error(L, "host callback requires nil, number or string arguments");
    }
    /* The Rust frame is gone before any allocating Lua API/error call below. */
    status = runtime->handler(runtime->context, id, values, (size_t)count,
                              &result, runtime->error, sizeof(runtime->error));
    if (status < 0) {
        runtime->error[sizeof(runtime->error) - 1] = 0;
        lua_error(L, runtime->error);
    }
    if (status == 1) { lua_pushnumber(L, result); return 1; }
    return 0;
}

/* Tag methods receive the property table first. Its tag selects this closure's
 * callback IDs; Rust sees only the property key and optional assigned value. */
static int host_property(lua_State *L) {
    if (lua_gettop(L) == 4) {
        lua_pushvalue(L, 2);
        lua_rawget(L, 1);
        if (lua_type(L, -1) != LUA_TNIL) return 1;
        lua_pop(L, 1);
    }
    lua_remove(L, 1);
    if (lua_gettop(L) == 4) {
        /* Original numeric property setters use lua_tonumber, including string
         * conversion and zero for nonnumeric values. Keep that inside C. */
        double number = lua_tonumber(L, 2);
        lua_pushnumber(L, number);
        lua_remove(L, 2);
        lua_insert(L, 2);
    }
    return host_call(L);
}

static int indexed_method_call(lua_State *L, int optional_tail) {
    int i, count = lua_gettop(L) - 3;
    double index = lua_tonumber(L, -1);
    if (lua_gettop(L) < 4 || lua_type(L, 1) != LUA_TTABLE)
        lua_error(L, "indexed method requires a table receiver");
    for (i = 2; i <= count; ++i) {
        int keep_number = !optional_tail || i == 2 || lua_type(L, i) == LUA_TNUMBER;
        double value = lua_tonumber(L, i);
        lua_remove(L, i);
        if (keep_number) lua_pushnumber(L, value);
        else lua_pushnil(L);
        lua_insert(L, i);
    }
    lua_pop(L, 1);
    lua_remove(L, 1);
    lua_pushnumber(L, index);
    lua_insert(L, 1);
    return host_call(L);
}

static int host_indexed_method(lua_State *L) { return indexed_method_call(L, 0); }
static int host_indexed_number_first(lua_State *L) { return indexed_method_call(L, 1); }

static int host_number_first(lua_State *L) {
    /* Retain Lua4's own tonumber conversion, including numeric strings and
     * nil/nonnumeric zero, before entering the Rust callback. */
    int i, count = lua_gettop(L) - 2;
    double value = count ? lua_tonumber(L, 1) : 0;
    if (count) lua_remove(L, 1);
    lua_pushnumber(L, value);
    lua_insert(L, 1);
    for (i = 2; i <= count; ++i) {
        if (lua_type(L, i) != LUA_TNUMBER) {
            lua_remove(L, i);
            lua_pushnil(L);
            lua_insert(L, i);
        }
    }
    return host_call(L);
}

/* Fixed original callback signatures, using Lua's own conversions. Closure
 * values stay rooted while strings are converted and allocation may occur. */
static int host_coerced(lua_State *L) {
    int count = lua_gettop(L) - 3, i;
    const char *shape = lua_tostring(L, -1);
    int required = (int)strlen(shape);
    /* Original4daa60 uses lua_tostring(L,-1), discarding earlier arguments. */
    if (shape[0] == 'S' && shape[1] == 0)
        while (count > 1) { lua_remove(L, 1); --count; }
    while (count > required) lua_remove(L, count--);
    while (count < required) {
        lua_pushnil(L);
        lua_insert(L, ++count);
    }
    for (i = 0; i < required; ++i) {
        if (shape[i] == 'n') lua_pushnumber(L, lua_tonumber(L, i + 1));
        else if (shape[i] == 'N') {
            if (lua_isnumber(L, i + 1)) lua_pushnumber(L, lua_tonumber(L, i + 1));
            else lua_pushnil(L);
        }
        else {
            const char *value = lua_tostring(L, i + 1);
            if (value) lua_pushlstring(L, value, lua_strlen(L, i + 1));
            else lua_pushnil(L);
        }
        lua_remove(L, i + 1);
        lua_insert(L, i + 1);
    }
    lua_pop(L, 1);
    return host_call(L);
}

/* Supplied Windows asset paths are case-insensitive. Build the lookup key
 * using Lua-owned strings, so allocation failure cannot leak host storage. */
static void push_script_key(lua_State *L, const char *path, size_t length) {
    char part[256];
    size_t at = 0, n, i;
    lua_pushstring(L, "");
    while (at < length) {
        n = length - at;
        if (n > sizeof(part)) n = sizeof(part);
        for (i = 0; i < n; ++i) {
            unsigned char c = (unsigned char)path[at + i];
            part[i] = c == '\\' ? '/' : (c >= 'A' && c <= 'Z' ? c + ('a' - 'A') : c);
        }
        lua_pushlstring(L, part, n);
        lua_concat(L, 2);
        at += n;
    }
}

static int run_script_buffer(lua_State *L) {
    mt4_runtime *runtime = (mt4_runtime *)lua_touserdata(L, -1);
    int argument = lua_gettop(L) - 1;
    const char *path = argument ? lua_tostring(L, argument) : NULL;
    int status;
    if (!path) lua_error(L, "script path must be a string");
    lua_getref(L, runtime->scripts);
    push_script_key(L, path, lua_strlen(L, argument));
    lua_gettable(L, -2);
    if (lua_type(L, -1) != LUA_TSTRING) lua_error(L, "script buffer is not registered");
    /* This nested parse/call stays entirely in C. The active Rust handler is
     * used only for ordinary callbacks and is never crossed by a longjmp. */
    status = lua_dobuffer(L, lua_tostring(L, -1), lua_strlen(L, -1), path);
    if (status) lua_error(L, runtime->error[0] ? runtime->error : "nested script failed");
    return 0;
}

extern int32_t openmt_lua4_ftol(double value);

/* Original004d9870: test timer availability before converting the final Lua
 * argument, then restore the four time constants after timer reconfiguration. */
static int calendar_frequency(lua_State *L) {
    mt4_runtime *runtime = (mt4_runtime *)lua_touserdata(L, -2);
    uint32_t id = (uint32_t)lua_tonumber(L, -1);
    int count = lua_gettop(L) - 2, status, i;
    double enabled = 0, ignored = 0;
    mt4_value value;
    const char *names[] = {"GAME_MINUTE", "GAME_HOUR", "GAME_YEAR", "GAME_DECADE"};
    const double numbers[] = {10, 625, 15000, 150000};
    if (!runtime->handler) lua_error(L, "calendar binding outside active operation");
    status = runtime->handler(runtime->context, id, &value, 0, &enabled,
                              runtime->error, sizeof(runtime->error));
    if (status < 0) lua_error(L, runtime->error);
    if (status != 1) lua_error(L, "calendar timer availability is unavailable");
    if (enabled == 0) return 0;
    lua_settop(L, count);
    memset(&value, 0, sizeof(value));
    value.kind = LUA_TNUMBER;
    value.number = count ? lua_tonumber(L, -1) : 0;
    status = runtime->handler(runtime->context, id + 1, &value, 1, &ignored,
                              runtime->error, sizeof(runtime->error));
    if (status < 0) lua_error(L, runtime->error);
    for (i = 0; i < 4; ++i) {
        lua_pushnumber(L, numbers[i]);
        lua_setglobal(L, names[i]);
    }
    return 0;
}

static void push_event_function(lua_State *L, const char *prefix) {
    lua_pushstring(L, prefix);
    lua_pushvalue(L, 1);
    lua_concat(L, 2);
    lua_getglobal(L, lua_tostring(L, -1));
    lua_remove(L, -2);
}

/* 004d5360/004d53b0 -> 004d5d00. Nested Lua initialization and immediate
 * callbacks stay in C, never calling a Lua API across an active Rust frame. */
static int trigger_event(lua_State *L) {
    mt4_runtime *runtime = (mt4_runtime *)lua_touserdata(L, -3);
    uint32_t id = (uint32_t)lua_tonumber(L, -2);
    int absolute = (int)lua_tonumber(L, -1);
    int count = lua_gettop(L) - 3;
    int32_t argument, delay;
    int status, reference;
    double clock = 0, ignored = 0;
    mt4_value values[3];
    lua_settop(L, count);
    if (count < 3) lua_settop(L, 3);
    if (!lua_tostring(L, 1)) lua_error(L, "event name must convert to a string");
    delay = openmt_lua4_ftol(lua_tonumber(L, 2));
    argument = openmt_lua4_ftol(lua_tonumber(L, 3));
    if (!runtime->handler) lua_error(L, "event trigger outside active operation");
    memset(values, 0, sizeof(values));
    if (absolute) {
        status = runtime->handler(runtime->context, id, values, 0, &clock,
                                  runtime->error, sizeof(runtime->error));
        if (status < 0) lua_error(L, runtime->error);
        if (status != 1) lua_error(L, "event total clock is unavailable");
        /* Unsigned subtraction reproduces the native low-word wrap. */
        delay = (int32_t)((uint32_t)delay - (uint32_t)openmt_lua4_ftol(clock));
        if (delay < 0) return 0;
    }
    push_event_function(L, "EVENTINITIALISE_");
    if (lua_type(L, -1) == LUA_TFUNCTION) {
        lua_pushnumber(L, argument);
        if (lua_call(L, 1, 0)) lua_error(L, runtime->error);
    } else lua_pop(L, 1);
    push_event_function(L, "EVENT_");
    if (lua_type(L, -1) != LUA_TFUNCTION) return 0;
    if (delay == 0) {
        lua_pushvalue(L, -1);
        lua_pushnumber(L, argument);
        if (lua_call(L, 1, 1)) lua_error(L, runtime->error);
        delay = openmt_lua4_ftol(lua_tonumber(L, -1));
        lua_pop(L, 1);
        if (delay == 0) return 0;
    }
    reference = lua_ref(L, 0);
    values[0].kind = values[1].kind = values[2].kind = LUA_TNUMBER;
    values[0].number = reference;
    values[1].number = (uint32_t)delay;
    values[2].number = argument;
    status = runtime->handler(runtime->context, id + 1, values, 3, &ignored,
                              runtime->error, sizeof(runtime->error));
    if (status < 0) {
        lua_unref(L, reference);
        lua_error(L, runtime->error);
    }
    return 0;
}

static void identity_bytes(lua_State *L, mt4_operation *op, const void *bytes, size_t length) {
    mt4_value value;
    double ignored = 0;
    int status;
    memset(&value, 0, sizeof(value));
    value.kind = LUA_TSTRING; value.bytes = (const unsigned char *)bytes; value.length = length;
    status = op->runtime->handler(op->runtime->context, 1, &value, 1, &ignored,
                                 op->runtime->error, sizeof(op->runtime->error));
    if (status < 0) lua_error(L, op->runtime->error);
}
static void identity_word(lua_State *L, mt4_operation *op, uint32_t word) {
    unsigned char bytes[4];
    int i;
    for (i = 0; i < 4; ++i) bytes[i] = (unsigned char)(word >> (i * 8));
    identity_bytes(L, op, bytes, 4);
}
/* Canonical semantic body: all opcodes, numeric/string constants, nested
 * prototypes and invocation shape. Excludes process pointers and debug paths. */
static void identity_proto(lua_State *L, mt4_operation *op, Proto *p, int depth) {
    int i, j;
    if (depth > 64 || p->ncode < 0 || p->ncode > 1000000) lua_error(L, "event prototype exceeds snapshot bound");
    identity_word(L, op, p->numparams); identity_word(L, op, p->is_vararg);
    identity_word(L, op, p->maxstacksize); identity_word(L, op, p->ncode);
    for (i = 0; i < p->ncode; ++i) identity_word(L, op, p->code[i]);
    identity_word(L, op, p->nknum);
    for (i = 0; i < p->nknum; ++i) {
        uint64_t bits;
        unsigned char bytes[8];
        if (sizeof(Number) != 8) lua_error(L, "event number format is not binary64");
        memcpy(&bits, &p->knum[i], 8);
        for (j = 0; j < 8; ++j) bytes[j] = (unsigned char)(bits >> (j * 8));
        identity_bytes(L, op, bytes, 8);
    }
    identity_word(L, op, p->nkstr);
    for (i = 0; i < p->nkstr; ++i) {
        if (p->kstr[i]->len > 0xffffffffU) lua_error(L, "event constant exceeds DWORD");
        identity_word(L, op, (uint32_t)p->kstr[i]->len);
        identity_bytes(L, op, p->kstr[i]->str, p->kstr[i]->len);
    }
    identity_word(L, op, p->nkproto);
    for (i = 0; i < p->nkproto; ++i) identity_proto(L, op, p->kproto[i], depth + 1);
}
static void protected_operation(lua_State *L, void *data) {
    mt4_operation *op = (mt4_operation *)data;
    size_t i;
    switch (op->action) {
    case 0:
        lua_pushuserdata(L, op->runtime);
        lua_pushcclosure(L, capture_error, 1);
        lua_setglobal(L, LUA_ERRORMESSAGE);
        lua_newtable(L);
        op->runtime->scripts = lua_ref(L, 1);
        break;
    case 1:
        lua_pushnumber(L, op->number);
        lua_setglobal(L, op->name);
        break;
    case 2:
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, (double)op->id);
        lua_pushcclosure(L, host_call, 2);
        lua_setglobal(L, op->name);
        break;
    case 3:
        /* Lua4 parse_buffer peeks at buff[0] even for a zero-byte chunk.
         * Rust empty slices may use a dangling non-null sentinel. */
        op->status = lua_dobuffer(L, op->length ? op->source : "", op->length, op->name);
        break;
    case 4:
        if (op->count > 256) lua_error(L, "event argument limit");
        lua_getglobal(L, op->name);
        for (i = 0; i < op->count; ++i) lua_pushnumber(L, op->arguments[i]);
        op->status = lua_call(L, (int)op->count, 0);
        break;
    case 12:
        if (op->count > 256) lua_error(L, "event argument limit");
        lua_getglobal(L, op->name);
        for (i = 0; i < op->count; ++i) lua_pushnumber(L, op->arguments[i]);
        op->status = lua_call(L, (int)op->count, 1);
        if (!op->status) op->number = lua_tonumber(L, -1);
        break;
    case 13:
        lua_getglobal(L, op->name);
        if (lua_type(L, -1) == LUA_TFUNCTION) {
            op->id = (uint32_t)lua_ref(L, 0);
            op->present = 1;
        }
        break;
    case 14:
        if (op->id > 0x7fffffffU || !lua_getref(L, (int)op->id)) break;
        op->present = 1;
        lua_pushnumber(L, op->number);
        op->status = lua_call(L, 1, 1);
        if (!op->status) op->number = lua_tonumber(L, -1);
        break;
    case 15:
        /* lua_unref assumes a valid occupied slot. Check before entering it;
         * collected weak references remain occupied and can be released. */
        if (op->id >= (uint32_t)L->refSize || L->refArray[op->id].st > HOLD)
            lua_error(L, "invalid or released Lua reference");
        lua_unref(L, (int)op->id);
        break;
    case 19: {
        const char *best = NULL;
        size_t length = 0;
        mt4_value value;
        double ignored = 0;
        int status;
        Closure *closure;
        if (op->id > 0x7fffffffU || !lua_getref(L, (int)op->id)) break;
        if (lua_type(L, -1) != LUA_TFUNCTION) break;
        closure = (Closure *)lua_topointer(L, -1);
        if (closure->isC || closure->nupvalues != 0) lua_error(L, "event save requires a named Lua function without captured upvalues");
        /* Snapshot only named original EVENT_ globals. Numeric VM references
         * and anonymous/replaced closures are never portable save identities. */
        lua_getglobals(L);
        lua_pushnil(L);
        while (lua_next(L, 2)) {
            if (lua_type(L, -2) == LUA_TSTRING && lua_equal(L, 1, -1)) {
                const char *name = lua_tostring(L, -2);
                size_t n = lua_strlen(L, -2);
                if (n >= 6 && !memcmp(name, "EVENT_", 6) &&
                    (!best || strcmp(name, best) < 0)) { best = name; length = n; }
            }
            lua_pop(L, 1);
        }
        if (!best) break;
        if (!op->runtime->handler) lua_error(L, "reference name outside protected handler");
        memset(&value, 0, sizeof(value));
        value.kind = LUA_TSTRING; value.bytes = (const unsigned char *)best; value.length = length;
        status = op->runtime->handler(op->runtime->context, 0, &value, 1, &ignored,
                                     op->runtime->error, sizeof(op->runtime->error));
        if (status < 0) lua_error(L, op->runtime->error);
        identity_bytes(L, op, "OMT2LUAF", 8);
        identity_word(L, op, 1);
        identity_proto(L, op, closure->f.l, 0);
        op->present = 1;
        break;
    }
    case 18: {
        char shape[17];
        if (op->count != (size_t)-1 && op->count > 16) lua_error(L, "numeric method argument limit");
        lua_getglobal(L, op->name);
        if (lua_type(L, -1) != LUA_TTABLE) lua_error(L, "numeric method target is not a table");
        lua_pushstring(L, op->source);
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, op->id);
        if (op->count == (size_t)-1) lua_pushcclosure(L, host_number_first, 2);
        else {
            memset(shape, 'n', op->count); shape[op->count] = 0;
            lua_pushstring(L, shape);
            lua_pushcclosure(L, host_coerced, 3);
        }
        lua_rawset(L, -3);
        break;
    }
    case 17:
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, op->id);
        lua_pushcclosure(L, calendar_frequency, 2);
        lua_setglobal(L, op->name);
        break;
    case 16:
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, op->id);
        lua_pushnumber(L, op->present);
        lua_pushcclosure(L, trigger_event, 3);
        lua_setglobal(L, op->name);
        break;
    case 20:
        lua_getglobal(L, op->name);
        if (lua_type(L, -1) != LUA_TTABLE) lua_error(L, "calendar method target is not a table");
        lua_pushstring(L, op->source);
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, op->id);
        lua_pushcclosure(L, calendar_frequency, 2);
        lua_rawset(L, -3);
        break;
    case 6: {
        int tag = lua_newtag(L);
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, (double)op->id);
        lua_pushcclosure(L, host_property, 2);
        lua_settagmethod(L, tag, "gettable");
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, (double)op->id + 1);
        lua_pushcclosure(L, host_property, 2);
        lua_settagmethod(L, tag, "settable");
        lua_newtable(L);
        lua_settag(L, tag);
        lua_setglobal(L, op->name);
        break;
    }
    case 5:
        lua_getglobal(L, op->name);
        if (lua_type(L, -1) == LUA_TNUMBER) {
            op->number = lua_tonumber(L, -1);
            op->present = 1;
        } else if (lua_type(L, -1) != LUA_TNIL) {
            lua_error(L, "global is not numeric");
        }
        break;
    case 7:
        lua_pushuserdata(L, op->runtime);
        lua_pushcclosure(L, run_script_buffer, 1);
        lua_setglobal(L, op->name);
        break;
    case 8:
        lua_getref(L, op->runtime->scripts);
        push_script_key(L, op->name, strlen(op->name));
        lua_pushlstring(L, op->length ? op->source : "", op->length);
        lua_settable(L, -3);
        break;
    case 9:
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, (double)op->id);
        lua_pushcclosure(L, host_number_first, 2);
        lua_setglobal(L, op->name);
        break;
    case 11:
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, (double)op->id);
        lua_pushstring(L, op->source);
        lua_pushcclosure(L, host_coerced, 3);
        lua_setglobal(L, op->name);
        break;
    case 10:
        lua_getglobal(L, op->name);
        if (lua_type(L, -1) != LUA_TTABLE) lua_error(L, "method target is not a table");
        lua_pushstring(L, op->source);
        lua_pushuserdata(L, op->runtime);
        lua_pushnumber(L, (double)op->id);
        lua_pushnumber(L, op->number);
        lua_pushcclosure(L, op->present ? host_indexed_number_first : host_indexed_method, 3);
        lua_rawset(L, -3);
        break;
    }
}

static int perform(mt4_operation *op) {
    mt4_runtime *runtime = op->runtime;
    int status;
    runtime->error[0] = 0;
    status = luaD_runprotected(runtime->state, protected_operation, op);
    lua_settop(runtime->state, 0);
    return status ? status : op->status;
}

mt4_runtime *mt4_new(void) {
    mt4_runtime *runtime = (mt4_runtime *)calloc(1, sizeof(mt4_runtime));
    mt4_operation op;
    if (!runtime) return NULL;
    runtime->state = lua_open(1024);
    if (!runtime->state) { free(runtime); return NULL; }
    memset(&op, 0, sizeof(op)); op.runtime = runtime;
    if (perform(&op)) { lua_close(runtime->state); free(runtime); return NULL; }
    return runtime;
}
void mt4_free(mt4_runtime *runtime) {
    lua_close(runtime->state); free(runtime);
}
const char *mt4_error(mt4_runtime *runtime) { return runtime->error; }
int mt4_set_number(mt4_runtime *runtime, const char *name, double value) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 1;
    op.name = name; op.number = value; return perform(&op);
}
int mt4_register(mt4_runtime *runtime, const char *name, uint32_t id) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 2;
    op.name = name; op.id = id; return perform(&op);
}
int mt4_register_properties(mt4_runtime *runtime, const char *name, uint32_t id) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 6;
    op.name = name; op.id = id; return perform(&op);
}
int mt4_script_buffer(mt4_runtime *runtime, const char *path, const char *source, size_t length) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 8;
    op.name = path; op.source = source; op.length = length; return perform(&op);
}
int mt4_script_loader(mt4_runtime *runtime, const char *name) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 7;
    op.name = name; return perform(&op);
}
int mt4_register_number_first(mt4_runtime *runtime, const char *name, uint32_t id) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 9;
    op.name = name; op.id = id; return perform(&op);
}
int mt4_register_coerced(mt4_runtime *runtime, const char *name, uint32_t id, const char *shape) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 11;
    op.name = name; op.id = id; op.source = shape; return perform(&op);
}
int mt4_indexed_method(mt4_runtime *runtime, const char *table, const char *method, uint32_t id, uint32_t index, int optional_tail) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 10;
    op.name = table; op.source = method; op.id = id; op.number = index; op.present = optional_tail; return perform(&op);
}
int mt4_run(mt4_runtime *runtime, const char *name, const char *source, size_t length,
            const double *arguments, size_t count, mt4_handler handler, void *context) {
    mt4_operation op;
    int status;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = source ? 3 : 4;
    op.name = name; op.source = source; op.length = length;
    op.arguments = arguments; op.count = count;
    runtime->handler = handler; runtime->context = context;
    status = perform(&op);
    runtime->handler = NULL; runtime->context = NULL;
    return status;
}
int mt4_number(mt4_runtime *runtime, const char *name, double *number, int *present) {
    mt4_operation op;
    int status;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 5; op.name = name;
    status = perform(&op); *number = op.number; *present = op.present; return status;
}
int mt4_call_number(mt4_runtime *runtime, const char *name,
                    const double *arguments, size_t count, mt4_handler handler,
                    void *context, double *number) {
    mt4_operation op;
    int status;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 12;
    op.name = name; op.arguments = arguments; op.count = count;
    runtime->handler = handler; runtime->context = context;
    status = perform(&op);
    runtime->handler = NULL; runtime->context = NULL;
    *number = op.number;
    return status;
}
int mt4_reference(mt4_runtime *runtime, int action, const char *name,
                  uint32_t *reference, double *number, int *present,
                  mt4_handler handler, void *context) {
    mt4_operation op;
    int status;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = action;
    op.name = name; op.id = *reference; op.number = *number;
    runtime->handler = handler; runtime->context = context;
    status = perform(&op);
    runtime->handler = NULL; runtime->context = NULL;
    *reference = op.id; *number = op.number; *present = op.present;
    return status;
}
int mt4_event_binding(mt4_runtime *runtime, const char *name, uint32_t id, int absolute) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 16;
    op.name = name; op.id = id; op.present = absolute;
    return perform(&op);
}

/* Native-comparison adapter: execute the vendored table helpers against a
 * small supplied numeric table, independent of host pointer width/layout. */
int mt4_hash_probe(int op, double value, int present, double lookup, int mismatch) {
    Hash table;
    Node nodes[8];
    TObject key_value;
    const TObject *result;
    int i, index;
    memset(&table, 0, sizeof(table)); memset(nodes, 0, sizeof(nodes));
    table.node = nodes; table.size = 8;
    for (i = 0; i < 8; ++i) nodes[i].key.ttype = LUA_TNIL;
    key_value.ttype = LUA_TNUMBER; key_value.value.n = value;
    if (op == 0) return (int)(luaH_mainposition(&table, &key_value) - nodes);
    if (present) {
        TObject stored;
        stored.ttype = LUA_TNUMBER; stored.value.n = lookup;
        index = (int)(luaH_mainposition(&table, &stored) - nodes);
        nodes[index].key = stored;
        if (mismatch) nodes[index].key.value.n += 0.25;
    }
    result = luaH_getnum(&table, value);
    for (i = 0; i < 8; ++i) if (result == &nodes[i].val) return i;
    return -1;
}

int mt4_calendar_binding(mt4_runtime *runtime, const char *name, uint32_t id) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 17;
    op.name = name; op.id = id; return perform(&op);
}

int mt4_number_method(mt4_runtime *runtime, const char *table, const char *method, uint32_t id, size_t count) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 18;
    op.name = table; op.source = method; op.id = id; op.count = count;
    return perform(&op);
}

int mt4_calendar_method(mt4_runtime *runtime, const char *table, const char *method, uint32_t id) {
    mt4_operation op;
    memset(&op, 0, sizeof(op)); op.runtime = runtime; op.action = 20;
    op.name = table; op.source = method; op.id = id;
    return perform(&op);
}
