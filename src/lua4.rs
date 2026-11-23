//! Buffer-only Lua 4.0 embedding. Game callbacks return before C raises errors.
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Number(f64),
    Bytes(Vec<u8>),
}

/// Original508c20/508c80 call50e928 for numeric-key hashing. Keep the same
/// low-word result on LP64 and for values outside the signed32-bit range.
#[unsafe(no_mangle)]
pub extern "C" fn openmt_lua4_number_hash(value: f64) -> u32 {
    crate::economy::ftol_low_word(value) as u32
}
#[unsafe(no_mangle)]
pub extern "C" fn openmt_lua4_ftol(value: f64) -> i32 {
    crate::economy::ftol_low_word(value)
}

type Handler<'a> = dyn FnMut(u32, &[Value]) -> Result<Option<f64>, String> + 'a;
type EventHandler<'a> =
    dyn FnMut(u32, &[Value], &mut crate::script_events::Events) -> Result<Option<f64>, String> + 'a;
struct Bridge<'a> {
    handler: &'a mut Handler<'a>,
}

#[repr(C)]
struct RawValue {
    kind: c_int,
    number: f64,
    bytes: *const u8,
    length: usize,
}
type Callback = unsafe extern "C" fn(
    *mut c_void,
    u32,
    *const RawValue,
    usize,
    *mut f64,
    *mut c_char,
    usize,
) -> c_int;
unsafe extern "C" {
    fn mt4_event_binding(
        runtime: *mut c_void,
        name: *const c_char,
        id: u32,
        absolute: c_int,
    ) -> c_int;
    fn mt4_new() -> *mut c_void;
    fn mt4_free(runtime: *mut c_void);
    fn mt4_error(runtime: *mut c_void) -> *const c_char;
    fn mt4_set_number(runtime: *mut c_void, name: *const c_char, value: f64) -> c_int;
    fn mt4_register_properties(runtime: *mut c_void, name: *const c_char, id: u32) -> c_int;
    fn mt4_calendar_binding(runtime: *mut c_void, name: *const c_char, id: u32) -> c_int;
    fn mt4_calendar_method(runtime: *mut c_void, table: *const c_char,
        method: *const c_char, id: u32) -> c_int;
    fn mt4_number_method(
        runtime: *mut c_void,
        table: *const c_char,
        method: *const c_char,
        id: u32,
        count: usize,
    ) -> c_int;
    fn mt4_register(runtime: *mut c_void, name: *const c_char, id: u32) -> c_int;
    fn mt4_register_coerced(
        runtime: *mut c_void,
        name: *const c_char,
        id: u32,
        shape: *const c_char,
    ) -> c_int;
    fn mt4_register_number_first(runtime: *mut c_void, name: *const c_char, id: u32) -> c_int;
    fn mt4_indexed_method(
        runtime: *mut c_void,
        table: *const c_char,
        method: *const c_char,
        id: u32,
        index: u32,
        optional_tail: c_int,
    ) -> c_int;
    fn mt4_script_loader(runtime: *mut c_void, name: *const c_char) -> c_int;
    fn mt4_script_buffer(
        runtime: *mut c_void,
        path: *const c_char,
        source: *const c_char,
        length: usize,
    ) -> c_int;
    fn mt4_number(
        runtime: *mut c_void,
        name: *const c_char,
        number: *mut f64,
        present: *mut c_int,
    ) -> c_int;
    fn mt4_call_number(
        runtime: *mut c_void,
        name: *const c_char,
        arguments: *const f64,
        count: usize,
        handler: Callback,
        context: *mut c_void,
        number: *mut f64,
    ) -> c_int;
    fn mt4_reference(
        runtime: *mut c_void,
        action: c_int,
        name: *const c_char,
        reference: *mut u32,
        number: *mut f64,
        present: *mut c_int,
        handler: Callback,
        context: *mut c_void,
    ) -> c_int;
    fn mt4_run(
        runtime: *mut c_void,
        name: *const c_char,
        source: *const c_char,
        length: usize,
        arguments: *const f64,
        count: usize,
        handler: Callback,
        context: *mut c_void,
    ) -> c_int;
}

unsafe extern "C" fn dispatch(
    context: *mut c_void,
    id: u32,
    raw: *const RawValue,
    count: usize,
    number: *mut f64,
    error: *mut c_char,
    capacity: usize,
) -> c_int {
    // C owns the value array and Lua strings for this synchronous callback.
    // We copy them before invoking application code; no Lua API runs in Rust.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bridge = unsafe { &mut *context.cast::<Bridge<'_>>() };
        let raw = unsafe { std::slice::from_raw_parts(raw, count) };
        let values = raw
            .iter()
            .map(|value| match value.kind {
                1 => Value::Nil,
                2 => Value::Number(value.number),
                3 => Value::Bytes(
                    unsafe { std::slice::from_raw_parts(value.bytes, value.length) }.to_vec(),
                ),
                _ => unreachable!("C shim rejected unsupported value type"),
            })
            .collect::<Vec<_>>();
        (bridge.handler)(id, &values)
    }));
    let message = match result {
        Ok(Ok(Some(value))) => {
            unsafe {
                *number = value;
            }
            return 1;
        }
        Ok(Ok(None)) => return 0,
        Ok(Err(message)) => message,
        Err(_) => "host callback panicked".to_owned(),
    };
    // Fixed C-owned error buffer remains alive after this Rust frame returns.
    let length = message.len().min(capacity.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr(), error.cast::<u8>(), length);
        if capacity != 0 {
            *error.add(length) = 0;
        }
    }
    -1
}

/// A VM is confined to its owning simulation thread. No standard I/O or other
/// libraries are opened. Host callbacks currently accept scalar/string values
/// and return an optional number; unsupported value types produce Lua errors.
pub struct Runtime {
    raw: NonNull<c_void>,
    _single_thread: PhantomData<Rc<()>>,
}
impl Runtime {
    /// Advance the original countdown list once. Nested script admissions use
    /// the same list, preserving traversal order rather than buffering events.
    pub fn tick_events(
        &mut self,
        events: &mut crate::script_events::Events,
        handler: &mut EventHandler<'_>,
    ) -> Result<(), String> {
        self.tick_events_with_invocation(events,&mut |vm,event,events|{
            vm.call_reference(event.reference,event.argument,&mut |id,args|handler(id,args,events))
        })
    }
    /// A retained event invocation boundary, before reference release and the
    /// next due event. The caller owns any audited terminal-call continuation.
    pub fn tick_events_with_invocation(
        &mut self,
        events:&mut crate::script_events::Events,
        invoke:&mut impl FnMut(&mut Runtime,crate::script_events::Event,
            &mut crate::script_events::Events)->Result<Option<f64>,String>,
    )->Result<(),String>{
        type Invocation<'a> = dyn FnMut(&mut Runtime, crate::script_events::Event,
            &mut crate::script_events::Events) -> Result<Option<f64>, String> + 'a;
        struct Host<'a, 'b> {
            vm: &'a mut Runtime,
            invoke: &'b mut Invocation<'b>,
        }
        impl crate::script_events::Host for Host<'_, '_> {
            fn invoke(
                &mut self,
                event: crate::script_events::Event,
                events: &mut crate::script_events::Events,
            ) -> Result<Option<f64>, String> {
                (self.invoke)(self.vm,event,events)
            }
            fn release(&mut self, reference: u32) -> Result<(), String> {
                self.vm.release_reference(reference)
            }
        }
        events.tick(&mut Host { vm: self, invoke })
    }
    /// Original Calendar.SetFrequency binding. id queries primary timer
    /// availability; id+1 receives the final argument converted by Lua itself.
    pub fn register_calendar_frequency(&mut self, name: &str, id: u32) -> Result<(), String> {
        id.checked_add(1).ok_or("calendar callback ID overflow")?;
        let name = CString::new(name).map_err(|_| "NUL in calendar binding name")?;
        self.check(unsafe { mt4_calendar_binding(self.raw.as_ptr(), name.as_ptr(), id) })
    }
    /// Attach the same protected frequency closure directly to a tagged table.
    /// Lua-level assignment would invoke its numeric property setter.
    pub fn register_calendar_frequency_method(&mut self, table: &str, method: &str,
        id: u32) -> Result<(), String> {
        id.checked_add(1).ok_or("calendar callback ID overflow")?;
        let table = CString::new(table).map_err(|_| "NUL in calendar table name")?;
        let method = CString::new(method).map_err(|_| "NUL in calendar method name")?;
        self.check(unsafe { mt4_calendar_method(self.raw.as_ptr(), table.as_ptr(), method.as_ptr(), id) })
    }
    /// Register original relative/absolute trigger semantics. `id` requests
    /// the live total clock (no arguments, numeric result); `id+1` admits an
    /// event as (weak reference, unsigned delay bits, signed argument).
    /// The admission handler must append to its live script_events::Events.
    pub fn register_event_triggers(&mut self, id: u32) -> Result<(), String> {
        id.checked_add(1).ok_or("event callback ID overflow")?;
        for (name, absolute) in [("C_TriggerEvent", 0), ("C_TriggerEventAbsolute", 1)] {
            let name = CString::new(name).unwrap();
            self.check(unsafe {
                mt4_event_binding(self.raw.as_ptr(), name.as_ptr(), id, absolute)
            })?;
        }
        Ok(())
    }
    pub fn new() -> Result<Self, String> {
        let raw = NonNull::new(unsafe { mt4_new() }).ok_or("cannot allocate Lua 4.0 state")?;
        Ok(Self {
            raw,
            _single_thread: PhantomData,
        })
    }
    fn check(&self, status: c_int) -> Result<(), String> {
        if status == 0 {
            return Ok(());
        }
        let text = unsafe { CStr::from_ptr(mt4_error(self.raw.as_ptr())) }.to_string_lossy();
        if text.is_empty() {
            Err(format!("Lua 4.0 error {status}"))
        } else {
            Err(text.into_owned())
        }
    }
    pub fn set_number(&mut self, name: &str, number: f64) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "Lua global name contains NUL")?;
        self.check(unsafe { mt4_set_number(self.raw.as_ptr(), name.as_ptr(), number) })
    }
    pub fn register(&mut self, name: &str, id: u32) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "Lua function name contains NUL")?;
        self.check(unsafe { mt4_register(self.raw.as_ptr(), name.as_ptr(), id) })
    }
    /// Fixed native signature: `n` uses Lua tonumber and `s` uses tostring.
    /// `N` converts numeric values/strings and passes other values as Nil.
    /// Missing arguments are nil before conversion; extra arguments are ignored.
    /// A sole `S` converts the last supplied argument to string (original4daa60).
    /// Failed string conversion is passed as Nil so the owner can reject it safely.
    pub fn register_coerced(&mut self, name: &str, id: u32, shape: &str) -> Result<(), String> {
        if shape.len() > 256
            || (shape != "S" && !shape.bytes().all(|b| matches!(b, b'n' | b'N' | b's' | b'p')))
        {
            return Err("invalid Lua callback signature".into());
        }
        let name = CString::new(name).map_err(|_| "Lua function name contains NUL")?;
        let shape = CString::new(shape).unwrap();
        self.check(unsafe {
            mt4_register_coerced(self.raw.as_ptr(), name.as_ptr(), id, shape.as_ptr())
        })
    }
    /// Register a scalar callback whose first argument uses Lua4 tonumber.
    /// Later numbers are retained; other optional values become nil, matching
    /// callbacks that accept those arguments only when their type is numeric.
    pub fn register_number_first(&mut self, name: &str, id: u32) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "Lua function name contains NUL")?;
        self.check(unsafe { mt4_register_number_first(self.raw.as_ptr(), name.as_ptr(), id) })
    }
    /// Install a dot-call method on an existing (possibly tagged) table.
    /// Its fixed arguments use Lua tonumber; no table receiver is inserted.
    pub fn register_number_method(
        &mut self,
        table: &str,
        method: &str,
        id: u32,
        count: usize,
    ) -> Result<(), String> {
        let table = CString::new(table).map_err(|_| "NUL in method table")?;
        let method = CString::new(method).map_err(|_| "NUL in method name")?;
        self.check(unsafe {
            mt4_number_method(
                self.raw.as_ptr(),
                table.as_ptr(),
                method.as_ptr(),
                id,
                count,
            )
        })
    }
    /// Number-first dot method retaining optional argument type distinctions.
    pub fn register_number_first_method(
        &mut self,
        table: &str,
        method: &str,
        id: u32,
    ) -> Result<(), String> {
        self.register_number_method(table, method, id, usize::MAX)
    }
    /// Add a colon-call method to a global table, including a property table.
    /// The host receives the bound numeric index in place of the table receiver;
    /// remaining arguments use Lua4 tonumber before the Rust callback.
    pub fn register_indexed_method(
        &mut self,
        table: &str,
        method: &str,
        id: u32,
        index: u32,
    ) -> Result<(), String> {
        self.indexed_method(table, method, id, index, false)
    }
    /// Bound receiver and Lua-tonumber first argument; later arguments retain
    /// actual numbers and normalize other types to nil. AddCommodity uses this
    /// to distinguish an omitted/nonnumeric percentage from numeric zero.
    pub fn register_indexed_number_first(
        &mut self,
        table: &str,
        method: &str,
        id: u32,
        index: u32,
    ) -> Result<(), String> {
        self.indexed_method(table, method, id, index, true)
    }
    fn indexed_method(
        &mut self,
        table: &str,
        method: &str,
        id: u32,
        index: u32,
        optional_tail: bool,
    ) -> Result<(), String> {
        let table = CString::new(table).map_err(|_| "Lua table name contains NUL")?;
        let method = CString::new(method).map_err(|_| "Lua method name contains NUL")?;
        self.check(unsafe {
            mt4_indexed_method(
                self.raw.as_ptr(),
                table.as_ptr(),
                method.as_ptr(),
                id,
                index,
                i32::from(optional_tail),
            )
        })
    }
    /// Register a synchronous script loader backed only by supplied buffers.
    /// Nested scripts share this VM and its active host callback handler.
    pub fn register_script_loader(&mut self, name: &str) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "Lua function name contains NUL")?;
        self.check(unsafe { mt4_script_loader(self.raw.as_ptr(), name.as_ptr()) })
    }
    /// Copy a text script into the VM's buffer registry. Path matching folds
    /// ASCII case and slash direction; no host filesystem access is performed.
    pub fn script_buffer(&mut self, path: &str, source: &[u8]) -> Result<(), String> {
        if source.first() == Some(&27) {
            return Err("binary Lua chunks are not accepted by the text-script interface".into());
        }
        let path = CString::new(path).map_err(|_| "Lua script path contains NUL")?;
        self.check(unsafe {
            mt4_script_buffer(
                self.raw.as_ptr(),
                path.as_ptr(),
                source.as_ptr().cast(),
                source.len(),
            )
        })
    }
    /// Register a tagged table with host-backed numeric properties. Get uses `id` and receives
    /// the key; set uses `id+1` and receives key/coerced number, including repeated writes.
    pub fn register_number_properties(&mut self, name: &str, id: u32) -> Result<(), String> {
        id.checked_add(1)
            .ok_or("Lua property callback ID overflow")?;
        let name = CString::new(name).map_err(|_| "Lua property table name contains NUL")?;
        self.check(unsafe { mt4_register_properties(self.raw.as_ptr(), name.as_ptr(), id) })
    }
    pub fn number(&mut self, name: &str) -> Result<Option<f64>, String> {
        let name = CString::new(name).map_err(|_| "Lua global name contains NUL")?;
        let (mut number, mut present) = (0., 0);
        self.check(unsafe {
            mt4_number(self.raw.as_ptr(), name.as_ptr(), &mut number, &mut present)
        })?;
        Ok((present != 0).then_some(number))
    }
    pub fn execute(
        &mut self,
        name: &str,
        source: &[u8],
        handler: &mut Handler<'_>,
    ) -> Result<(), String> {
        if source.first() == Some(&27) {
            return Err("binary Lua chunks are not accepted by the text-script interface".into());
        }
        self.run(name, Some(source), &[], handler)
    }
    pub fn call(
        &mut self,
        name: &str,
        arguments: &[f64],
        handler: &mut Handler<'_>,
    ) -> Result<(), String> {
        self.run(name, None, arguments, handler)
    }
    /// Return the first result through Lua4 tonumber, as original script-event
    /// dispatch does before FTOL. Missing/nonnumeric results become zero;
    /// numeric strings convert, and additional results are discarded.
    pub fn call_number(
        &mut self,
        name: &str,
        arguments: &[f64],
        handler: &mut Handler<'_>,
    ) -> Result<f64, String> {
        let name = CString::new(name).map_err(|_| "Lua function name contains NUL")?;
        let mut bridge = Bridge { handler };
        let mut number = 0.;
        self.check(unsafe {
            mt4_call_number(
                self.raw.as_ptr(),
                name.as_ptr(),
                arguments.as_ptr(),
                arguments.len(),
                dispatch,
                (&mut bridge as *mut Bridge<'_>).cast(),
                &mut number,
            )
        })?;
        Ok(number)
    }
    /// Capture a weak function reference, as original004d5d00 does. The handle
    /// belongs to this VM and must be released after its scheduled event ends.
    /// Later global reassignment does not change the referenced function.
    pub fn reference_function(&mut self, name: &str) -> Result<Option<u32>, String> {
        let name = CString::new(name).map_err(|_| "Lua function name contains NUL")?;
        let (reference, _, present) =
            self.reference_operation(13, Some(&name), 0, 0., &mut |_, _| {
                Err("unexpected reference callback".into())
            })?;
        Ok(present.then_some(reference))
    }
    /// Resolve a captured weak function and convert its first result to a
    /// number. None indicates a collected or otherwise unresolved reference.
    pub fn call_reference(
        &mut self,
        reference: u32,
        argument: i32,
        handler: &mut Handler<'_>,
    ) -> Result<Option<f64>, String> {
        let (_, number, present) =
            self.reference_operation(14, None, reference, f64::from(argument), handler)?;
        Ok(present.then_some(number))
    }
    pub fn release_reference(&mut self, reference: u32) -> Result<(), String> {
        self.reference_operation(15, None, reference, 0., &mut |_, _| {
            Err("unexpected reference callback".into())
        })?;
        Ok(())
    }
    /// Resolve a currently bound named EVENT_ function without invoking it.
    /// Anonymous, collected and replaced closures have no portable save name.
    pub fn event_reference_name(&mut self, reference: u32) -> Result<Option<String>, String> {
        Ok(self
            .event_reference_identity(reference)?
            .map(|(name, _)| name))
    }
    /// Exact canonical body plus currently bound global identity, never a VM address.
    pub fn event_reference_identity(
        &mut self,
        reference: u32,
    ) -> Result<Option<(String, Vec<u8>)>, String> {
        let mut name = None;
        let mut body = Vec::new();
        let (_, _, present) =
            self.reference_operation(19, None, reference, 0., &mut |id, args| {
                let [Value::Bytes(bytes)] = args else {
                    return Err("invalid event-name callback".into());
                };
                if id == 0 {
                    name = Some(
                        std::str::from_utf8(bytes)
                            .map_err(|_| "event name is not UTF-8")?
                            .to_owned(),
                    );
                } else if id == 1 {
                    if body
                        .len()
                        .checked_add(bytes.len())
                        .is_none_or(|n| n > 16 * 1024 * 1024)
                    {
                        return Err("event body exceeds snapshot bound".into());
                    }
                    body.extend_from_slice(bytes);
                } else {
                    return Err("invalid event identity callback kind".into());
                }
                Ok(None)
            })?;
        if present && name.is_none() {
            return Err("event reference name was not delivered".into());
        }
        Ok(name.map(|name| (name, body)))
    }
    fn reference_operation(
        &mut self,
        action: c_int,
        name: Option<&CStr>,
        mut reference: u32,
        mut number: f64,
        handler: &mut Handler<'_>,
    ) -> Result<(u32, f64, bool), String> {
        let mut bridge = Bridge { handler };
        let mut present = 0;
        self.check(unsafe {
            mt4_reference(
                self.raw.as_ptr(),
                action,
                name.map_or(std::ptr::null(), CStr::as_ptr),
                &mut reference,
                &mut number,
                &mut present,
                dispatch,
                (&mut bridge as *mut Bridge<'_>).cast(),
            )
        })?;
        Ok((reference, number, present != 0))
    }
    fn run(
        &mut self,
        name: &str,
        source: Option<&[u8]>,
        arguments: &[f64],
        handler: &mut Handler<'_>,
    ) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "Lua chunk/function name contains NUL")?;
        let mut bridge = Bridge { handler };
        self.check(unsafe {
            mt4_run(
                self.raw.as_ptr(),
                name.as_ptr(),
                source.map_or(std::ptr::null(), |s| s.as_ptr().cast()),
                source.map_or(0, <[u8]>::len),
                arguments.as_ptr(),
                arguments.len(),
                dispatch,
                (&mut bridge as *mut Bridge<'_>).cast(),
            )
        })
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            mt4_free(self.raw.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_save_identity_rejects_replaced_closures_and_compares_complete_bodies() {
        let mut first = Runtime::new().unwrap();
        let mut second = Runtime::new().unwrap();
        let source = b"function EVENT_Save(a) return a+7 end";
        first
            .execute("first-path", source, &mut |_, _| Ok(None))
            .unwrap();
        second
            .execute("second-path", source, &mut |_, _| Ok(None))
            .unwrap();
        let a = first.reference_function("EVENT_Save").unwrap().unwrap();
        let b = second.reference_function("EVENT_Save").unwrap().unwrap();
        assert_eq!(
            first.event_reference_identity(a).unwrap(),
            second.event_reference_identity(b).unwrap()
        );
        second
            .execute(
                "changed",
                b"function EVENT_Save(a) return a+8 end",
                &mut |_, _| Ok(None),
            )
            .unwrap();
        let changed = second.reference_function("EVENT_Save").unwrap().unwrap();
        assert_ne!(
            first.event_reference_identity(a).unwrap(),
            second.event_reference_identity(changed).unwrap()
        );
        assert_eq!(
            second.event_reference_identity(b).unwrap(),
            None,
            "captured old closure must not rebind to replaced global"
        );
        first.release_reference(a).unwrap();
        second.release_reference(b).unwrap();
        second.release_reference(changed).unwrap();
    }
    #[test]
    fn timed_logic_script_pause_preserves_auxiliary_timers_and_current_iteration() {
        let mut vm = Runtime::new().unwrap();
        vm.register("Pause", 90).unwrap();
        vm.execute(
            "timer script",
            b"hits=0; function EVENT_Stop() hits=hits+1; Pause(); return 0 end",
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let reference = vm.reference_function("EVENT_Stop").unwrap().unwrap();
        let mut events = crate::script_events::Events::default();
        events.append(crate::script_events::Event {
            reference,
            remaining: 1,
            argument: 0,
        });
        let mut pause = crate::pause::State::default();
        let mut tasks = vec![[0; 20]; 2];
        crate::timers::configure(&mut tasks[0], 50., 0x4ba580);
        tasks[0][4..8].copy_from_slice(&1u32.to_le_bytes());
        crate::timers::configure(&mut tasks[1], 25., 0x481680);
        tasks[1][4..8].copy_from_slice(&10u32.to_le_bytes());
        let mut frame = crate::timers::Frame::default();
        frame.initialize(1000);
        let mut clock = crate::clock::Clock {
            calendar: [9, 0, 0, 9, 0],
            timing: [0, 0, 0, 0, 0, 1],
        };
        let mut controls = crate::scheduler::Controls {
            iterations: 2,
            abort: false,
            alternate_script_active: false,
            network_enabled: false,
            network_override: false,
            deadline: u32::MAX,
            gate_32: false,
            gate_32_offset: false,
            before_six_sound: 0,
        };
        let mut auxiliary = 0;
        for (now, total, maintenance) in [(200, 2, 10), (400, 2, 20), (440, 4, 22)] {
            if now == 440 {
                pause
                    .unpause(0, |event, _| {
                        event.apply(&mut tasks);
                        Ok::<_, String>(())
                    })
                    .unwrap();
            }
            crate::scheduler::advance_timed(
                Some(1000 + now),
                &mut frame,
                &mut tasks,
                &mut clock,
                &mut controls,
                |event, _, _, tasks| {
                    match event {
                        crate::scheduler::TimedEvent::AuxiliaryTimer(address) => {
                            assert_eq!(address, 0x4ba580);
                            auxiliary += 1;
                        }
                        crate::scheduler::TimedEvent::Logic(crate::scheduler::Event::Helper(
                            0x4d5b40,
                        )) => {
                            vm.tick_events(&mut events, &mut |id, _, _| {
                                assert_eq!(id, 90);
                                pause.pause(0, |event, _| {
                                    event.apply(tasks);
                                    Ok::<_, String>(())
                                })?;
                                Ok(None)
                            })?;
                        }
                        // This fixture tests timing and script dispatch only;
                        // resident, animation and other world helpers are seams.
                        _ => {}
                    }
                    Ok(0)
                },
            )
            .unwrap();
            assert_eq!(clock.timing[0], total);
            assert_eq!(clock.timing[1], total);
            assert_eq!(auxiliary, maintenance);
            assert_eq!(pause.has_reason(), now != 440);
            assert_eq!(tasks[0][0], 1);
        }
        assert_eq!(vm.number("hits").unwrap(), Some(1.));
        assert!(events.pending().is_empty());
    }

    #[test]
    fn calendar_frequency_preserves_native_gate_and_last_argument() {
        let mut vm = Runtime::new().unwrap();
        vm.register_calendar_frequency("SetFrequency", 70).unwrap();
        let mut enabled = false;
        let mut values = Vec::new();
        vm.execute(
            "disabled calendar",
            b"GAME_HOUR=7; SetFrequency('0.04')",
            &mut |id, _| {
                assert_eq!(id, 70);
                Ok(Some(f64::from(enabled)))
            },
        )
        .unwrap();
        assert_eq!(vm.number("GAME_HOUR").unwrap(), Some(7.));
        enabled = true;
        vm.execute("calendar coercion", b"result=SetFrequency(99,'0.04'); SetFrequency(nil); SetFrequency(); SetFrequency({}); SetFrequency(0.08)", &mut |id,args| {
            if id==70 { return Ok(Some(f64::from(enabled))); }
            assert_eq!(id,71);
            let [Value::Number(value)] = args else { panic!("calendar coercion"); };
            values.push(*value);
            Ok(None)
        }).unwrap();
        assert_eq!(values, vec![0.04, 0., 0., 0., 0.08]);
        assert_eq!(vm.number("result").unwrap(), None);
        for (name, value) in [
            ("GAME_MINUTE", 10.),
            ("GAME_HOUR", 625.),
            ("GAME_YEAR", 15000.),
            ("GAME_DECADE", 150000.),
        ] {
            assert_eq!(vm.number(name).unwrap(), Some(value));
        }
        assert!(
            vm.execute(
                "failed calendar",
                b"GAME_HOUR=8; SetFrequency(1)",
                &mut |id, _| {
                    if id == 70 {
                        Ok(Some(1.))
                    } else {
                        Err("host calendar error".into())
                    }
                }
            )
            .is_err()
        );
        assert_eq!(vm.number("GAME_HOUR").unwrap(), Some(8.));
    }

    #[test]
    fn original_trigger_order_absolute_filter_and_nested_initialization() {
        let mut vm = Runtime::new().unwrap();
        vm.register_event_triggers(50).unwrap();
        vm.register("Observe", 60).unwrap();
        let mut trace = vec![];
        let mut queued = vec![];
        let mut handler = |id, args: &[Value]| match id {
            50 => Ok(Some(100.)),
            51 => {
                queued.push(args.to_vec());
                Ok(None)
            }
            60 => {
                trace.push(args.to_vec());
                Ok(None)
            }
            _ => Err("unexpected call".into()),
        };
        vm.execute(
            "trigger order",
            br#"
            function EVENTINITIALISE_Child(a) Observe(1,a) end
            function EVENT_Child(a) Observe(2,a); return 0 end
            function EVENTINITIALISE_Parent(a)
                Observe(3,a); C_TriggerEvent('Child',0,a+1); Observe(4,a)
            end
            function EVENT_Parent(a) Observe(5,a); return '2.9' end
            C_TriggerEventAbsolute('Parent',99,10)
            C_TriggerEventAbsolute('Parent',100,'7.9')
            C_TriggerEventAbsolute('Child',103,20)
            C_TriggerEvent('Child',-1,-3)
            C_TriggerEvent('NoFunction',10,0)
        "#,
            &mut handler,
        )
        .unwrap();
        assert_eq!(
            trace,
            vec![
                vec![Value::Number(3.), Value::Number(7.)],
                vec![Value::Number(1.), Value::Number(8.)],
                vec![Value::Number(2.), Value::Number(8.)],
                vec![Value::Number(4.), Value::Number(7.)],
                vec![Value::Number(5.), Value::Number(7.)],
                vec![Value::Number(1.), Value::Number(20.)],
                vec![Value::Number(1.), Value::Number(-3.)],
            ]
        );
        assert_eq!(queued.len(), 3);
        for (args, delay, argument) in [
            (&queued[0], 2., 7.),
            (&queued[1], 3., 20.),
            (&queued[2], 4294967295., -3.),
        ] {
            assert_eq!(&args[1..], &[Value::Number(delay), Value::Number(argument)]);
            let Value::Number(reference) = args[0] else {
                panic!("not a reference")
            };
            vm.release_reference(reference as u32).unwrap();
        }
    }
    #[test]
    fn invocation_continuation_precedes_release_and_next_due_event() {
        let mut vm = Runtime::new().unwrap();
        vm.execute("ordered terminal events", b"consumed=0; observed=0; seen=-1; function EVENT_First(a) observed=1 end; function EVENT_Second(a) seen=consumed; observed=2 end", &mut |_, _| Ok(None)).unwrap();
        let first = vm.reference_function("EVENT_First").unwrap().unwrap();
        let second = vm.reference_function("EVENT_Second").unwrap().unwrap();
        let mut events = crate::script_events::Events::default();
        for reference in [first, second] {
            events.append(crate::script_events::Event { reference, remaining: 1, argument: 0 });
        }
        let mut order = vec![];
        vm.tick_events_with_invocation(&mut events, &mut |vm, event, _| {
            assert!(vm.event_reference_identity(event.reference)?.is_some());
            if event.reference == second {
                assert!(vm.event_reference_identity(first)?.is_none());
            }
            let result = vm.call_reference(event.reference, event.argument, &mut |_, _| Ok(None))?;
            let observed = vm.number("observed")?.unwrap() as u32;
            if event.reference == second {
                assert_eq!(vm.number("seen")?, Some(1.), "the second Lua event sees the first producer's completed consumer");
            }
            order.push(observed * 10);
            // This is the synchronous terminal producer consumer, after Lua
            // returns but while its event still owns the captured reference.
            vm.execute("consume terminal producer", if observed == 1 { b"consumed=1" } else { b"consumed=2" }, &mut |_, _| Ok(None))?;
            assert!(vm.event_reference_identity(event.reference)?.is_some());
            order.push(observed * 10 + 1);
            Ok(result)
        }).unwrap();
        assert_eq!(order, [10, 11, 20, 21]);
        assert!(events.pending().is_empty());
        assert!(vm.event_reference_identity(second).unwrap().is_none());
    }
    #[test]
    fn countdown_calls_captured_lua_function_and_releases_it() {
        struct Host(Runtime);
        impl crate::script_events::Host for Host {
            fn invoke(
                &mut self,
                event: crate::script_events::Event,
                _: &mut crate::script_events::Events,
            ) -> Result<Option<f64>, String> {
                self.0
                    .call_reference(event.reference, event.argument, &mut |_, _| {
                        Err("unexpected callback".into())
                    })
            }
            fn release(&mut self, reference: u32) -> Result<(), String> {
                self.0.release_reference(reference)
            }
        }
        let mut host = Host(Runtime::new().unwrap());
        host.0.execute("repeating event", b"calls=0; total=0; function Event(a) calls=calls+1; total=total+a; if calls==1 then return '2.9' end end; retained=Event", &mut |_, _| Ok(None)).unwrap();
        let reference = host.0.reference_function("Event").unwrap().unwrap();
        assert_eq!(host.0.reference_function("Absent").unwrap(), None);
        assert_eq!(host.0.reference_function("total").unwrap(), None);
        host.0
            .execute(
                "replace global",
                b"function Event(a) total=999 end",
                &mut |_, _| Ok(None),
            )
            .unwrap();
        let mut events = crate::script_events::Events::default();
        events.append(crate::script_events::Event {
            reference,
            remaining: 1,
            argument: -7,
        });
        events.tick(&mut host).unwrap();
        assert_eq!(events.pending()[0].remaining, 2);
        events.tick(&mut host).unwrap();
        assert_eq!(host.0.number("calls").unwrap(), Some(1.));
        events.tick(&mut host).unwrap();
        assert!(events.pending().is_empty());
        assert_eq!(host.0.number("calls").unwrap(), Some(2.));
        assert_eq!(host.0.number("total").unwrap(), Some(-14.));
        assert_eq!(
            host.0
                .call_reference(reference, 0, &mut |_, _| Ok(None))
                .unwrap(),
            None
        );
        assert!(host.0.release_reference(reference).is_err());
        assert!(host.0.release_reference(u32::MAX).is_err());
        assert_eq!(
            host.0
                .call_reference(u32::MAX, 0, &mut |_, _| Ok(None))
                .unwrap(),
            None
        );
    }
    #[test]
    fn event_numeric_results_preserve_coercion_and_recover_after_errors() {
        let mut runtime = Runtime::new().unwrap();
        runtime.register("Observe", 9).unwrap();
        runtime
            .execute(
                "events",
                br#"
            function Number(a) Observe(a); return a+0.75, 99 end
            function Text() return '-12.5' end
            function Missing() end
            function Table() return {} end
            function BadText() return 'abc' end
            function Failure() Observe(0) end
        "#,
                &mut |_, _| Ok(None),
            )
            .unwrap();
        let mut observed = vec![];
        let mut handler = |id, args: &[Value]| {
            assert_eq!(id, 9);
            if args == [Value::Number(0.)] {
                return Err("event failure".into());
            }
            observed.push(args.to_vec());
            Ok(None)
        };
        assert_eq!(
            runtime.call_number("Number", &[4.], &mut handler).unwrap(),
            4.75
        );
        assert_eq!(
            runtime.call_number("Text", &[], &mut handler).unwrap(),
            -12.5
        );
        for name in ["Missing", "Table", "BadText"] {
            assert_eq!(runtime.call_number(name, &[], &mut handler).unwrap(), 0.);
        }
        assert!(
            runtime
                .call_number("Failure", &[], &mut handler)
                .unwrap_err()
                .contains("event failure")
        );
        assert!(
            runtime
                .call_number("NotDefined", &[], &mut handler)
                .is_err()
        );
        assert_eq!(
            runtime.call_number("Number", &[8.], &mut handler).unwrap(),
            8.75
        );
        assert_eq!(
            observed,
            vec![vec![Value::Number(4.)], vec![Value::Number(8.)]]
        );
    }
    #[test]
    fn optional_numeric_signature_retains_missing_and_invalid_values() {
        let mut runtime = Runtime::new().unwrap();
        runtime.register_coerced("Objective", 4, "nnN").unwrap();
        let mut calls = vec![];
        runtime.execute("optional number", b"Objective('6','200'); Objective(6,200,0); Objective(6,200,'1'); Objective(6,200,'bad'); Objective(6,200,{})", &mut |_,args| { calls.push(args.to_vec()); Ok(None) }).unwrap();
        for (call, flag) in calls.iter().zip([
            Value::Nil,
            Value::Number(0.),
            Value::Number(1.),
            Value::Nil,
            Value::Nil,
        ]) {
            assert_eq!(call, &vec![Value::Number(6.), Value::Number(200.), flag]);
        }
    }

    #[test]
    fn fixed_signature_uses_lua_conversions_without_crossing_rust() {
        let mut runtime = Runtime::new().unwrap();
        runtime.register_coerced("Map", 7, "snn").unwrap();
        let mut calls = vec![];
        runtime
            .execute(
                "coercion",
                b"Map(12, '2.5', {}); Map('abc'); Map(nil, 'bad', 9, {})",
                &mut |id, args| {
                    assert_eq!(id, 7);
                    calls.push(args.to_vec());
                    Ok(None)
                },
            )
            .unwrap();
        assert_eq!(
            calls,
            vec![
                vec![
                    Value::Bytes(b"12".to_vec()),
                    Value::Number(2.5),
                    Value::Number(0.)
                ],
                vec![
                    Value::Bytes(b"abc".to_vec()),
                    Value::Number(0.),
                    Value::Number(0.)
                ],
                vec![Value::Nil, Value::Number(0.), Value::Number(9.)],
            ]
        );
    }

    #[test]
    fn indexed_optional_number_preserves_native_type_distinction() {
        let mut runtime = Runtime::new().unwrap();
        runtime.register_number_properties("Info", 20).unwrap();
        runtime
            .register_indexed_number_first("Info", "Add", 30, 7)
            .unwrap();
        let mut calls = Vec::new();
        runtime.execute("optional numeric arguments", b"Info:Add('3'); Info:Add('3',0); Info:Add(3,'50'); Info:Add(3,{}); Info:Add(3,nil); Info:Add(3,50)", &mut |id, args| {
            assert_eq!(id, 30);
            calls.push(args.to_vec());
            Ok(None)
        }).unwrap();
        for args in &calls {
            assert_eq!(&args[..2], &[Value::Number(7.), Value::Number(3.)]);
        }
        assert_eq!(calls[0].len(), 2);
        assert_eq!(calls[1][2], Value::Number(0.));
        for args in &calls[2..5] {
            assert_eq!(args[2], Value::Nil);
        }
        assert_eq!(calls[5][2], Value::Number(50.));
    }
    #[test]
    fn nested_script_buffers_share_state_order_and_recover_from_errors() {
        let mut runtime = Runtime::new().unwrap();
        runtime.register_script_loader("C_RunScript").unwrap();
        runtime.register("Observe", 1).unwrap();
        runtime.script_buffer("scripts/empty.lua", b"").unwrap();
        runtime
            .script_buffer(
                "scripts/inner.lua",
                b"value=value+2; Observe(value); C_RunScript('scripts/empty.lua')",
            )
            .unwrap();
        runtime.script_buffer("scripts/outer.lua", b"value=1; Observe(value); C_RunScript('SCRIPTS\\\\INNER.LUA'); Observe(value); value=value+4").unwrap();
        let mut observed = vec![];
        let mut handler = |id, args: &[Value]| {
            assert_eq!(id, 1);
            observed.push(args[0].clone());
            Ok(None)
        };
        for _ in 0..2 {
            runtime
                .execute(
                    "root",
                    b"C_RunScript('ignored earlier argument','scripts/outer.lua'); Observe(value)",
                    &mut handler,
                )
                .unwrap();
        }
        assert!(
            runtime
                .execute("missing", b"C_RunScript('absent.lua')", &mut handler)
                .is_err()
        );
        runtime
            .script_buffer("scripts/bad.lua", b"this is not Lua")
            .unwrap();
        assert!(
            runtime
                .execute("bad", b"C_RunScript('scripts/bad.lua')", &mut handler)
                .is_err()
        );
        runtime
            .script_buffer("scripts/bad.lua", b"value=19")
            .unwrap();
        runtime
            .execute(
                "recovery",
                b"C_RunScript('scripts/bad.lua'); Observe(value)",
                &mut handler,
            )
            .unwrap();
        assert_eq!(
            observed,
            [1., 3., 3., 7., 1., 3., 3., 7., 19.].map(Value::Number)
        );
        assert_eq!(runtime.number("value").unwrap(), Some(19.));
        assert!(runtime.script_buffer("binary", &[27, 0]).is_err());
    }
    unsafe extern "C" {
        fn mt4_hash_probe(
            op: c_int,
            value: f64,
            present: c_int,
            lookup: f64,
            mismatch: c_int,
        ) -> c_int;
    }
    #[test]
    fn empty_script_buffer() {
        let mut runtime = Runtime::new().unwrap();
        runtime
            .execute("empty scenario", &[], &mut |_, _| {
                Err("unexpected callback".into())
            })
            .unwrap();
    }

    #[test]
    fn host_properties_preserve_reads_repeated_writes_and_error_recovery() {
        let mut runtime = Runtime::new().unwrap();
        runtime.register_number_properties("Scores", 20).unwrap();
        let mut value = 0.;
        let mut writes = 0;
        let mut handler = |id, args: &[Value]| {
            if args.first() != Some(&Value::Bytes(b"value".to_vec())) {
                return Err("unknown property".into());
            }
            if id == 20 {
                return Ok(Some(value));
            }
            let Some(Value::Number(number)) = args.get(1) else {
                return Err("numeric property required".into());
            };
            value = *number;
            writes += 1;
            Ok(None)
        };
        runtime
            .execute(
                "properties",
                b"Scores.value=4; Scores.value=Scores.value+3; answer=Scores.value",
                &mut handler,
            )
            .unwrap();
        assert!(
            runtime
                .execute("bad property", b"Scores.missing=4", &mut handler)
                .is_err()
        );
        runtime
            .execute(
                "recovery",
                b"Scores.value=\"9\"; answer=Scores.value",
                &mut handler,
            )
            .unwrap();
        assert_eq!(runtime.number("answer").unwrap(), Some(9.));
        assert_eq!(value, 9.);
        assert_eq!(writes, 3);
    }

    #[test]
    #[ignore = "requires original Lua numeric hash observations"]
    fn matches_native_numeric_table_helpers() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_LUA_NUMBER_HASH").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let double = |at| f64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTLUNUM2");
        assert_eq!(word(8), 2);
        assert_eq!(bytes.len(), 16 + word(12) as usize * 52);
        for case in 0..word(12) as usize {
            let at = 16 + case * 52;
            let value = double(at + 4);
            assert_eq!(openmt_lua4_number_hash(value), word(at + 40));
            assert_eq!(
                unsafe {
                    mt4_hash_probe(
                        word(at) as c_int,
                        value,
                        word(at + 12) as c_int,
                        double(at + 16),
                        word(at + 24) as c_int,
                    )
                },
                word(at + 36) as i32,
                "hash{case}"
            );
            assert_eq!(word(at + 48), 0);
        }
    }
    #[test]
    #[ignore = "requires supplied DEFAULT/ai.lua"]
    fn executes_supplied_housekeeping_function_through_native_checked_bindings() {
        let source =
            std::fs::read_to_string(std::env::var("OPENMT2001_AI_SCRIPT").unwrap()).unwrap();
        // Isolate this complete supplied function for the VM/binding boundary
        // check. This is not a production script loader or scenario bootstrap.
        let start = source
            .find("function EVENT_LoadHseKeepingTimetable")
            .unwrap();
        let end = start + source[start + 1..].find("\nfunction ").unwrap() + 1;
        let mut vm = Runtime::new().unwrap();
        // Original4d7346..4d73fc character registrations.
        for (index, name) in [
            "CAR", "BARROW", "DOG", "GUN", "HAT", "HORSE", "IRON", "SHIP", "SHOE", "THIMBLE",
        ]
        .iter()
        .enumerate()
        {
            vm.set_number(name, index as f64).unwrap();
        }
        for (name, value) in [
            ("HSE", 1.),
            ("TT_RENT", 1.),
            ("TT_EOD_STOCK_AND_PRICE", 13.),
            ("TT_TAKEOVER_BUS", 11.),
            ("TT_BUSINESS_STATES", 14.),
            ("TT_DEMOLISH", 5.),
            ("TT_REFURBISH_BUS", 12.),
            ("TT_SPOILAGE", 4.),
        ] {
            vm.set_number(name, value).unwrap();
        }
        vm.register("reset", 0).unwrap();
        vm.register("load", 1).unwrap();
        vm.execute("fixture binding", b"AiChars={}; for i=0,9 do AiChars[i]={index=i}; AiChars[i].ResetTimeTable=function(self,kind) reset(self.index,kind) end; AiChars[i].LoadTimeTable=function(self,kind,hour,minute,event) load(self.index,kind,hour,minute,event) end end", &mut |_,_| unreachable!()).unwrap();
        vm.execute(
            "DEFAULT housekeeping",
            &source.as_bytes()[start..end],
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut profiles = vec![[0u8; 0x650]; 10];
        for index in 0..10 {
            vm.call(
                "EVENT_LoadHseKeepingTimetable",
                &[index as f64],
                &mut |id, args| {
                    let numbers = args
                        .iter()
                        .map(|value| {
                            if let Value::Number(n) = value {
                                *n
                            } else {
                                panic!("non-numeric timetable argument")
                            }
                        })
                        .collect::<Vec<_>>();
                    let mut values = [0.; 4];
                    values[..numbers.len() - 1].copy_from_slice(&numbers[1..]);
                    crate::ai_events::configure_timetable(
                        &mut profiles[numbers[0] as usize],
                        id == 0,
                        values,
                    )?;
                    Ok(None)
                },
            )
            .unwrap();
        }
        let iron = crate::ai_events::Timetable::from_profile(&profiles[6], 1);
        assert_eq!(
            &iron.records[..4],
            &[
                crate::ai_events::Record {
                    timestamp: 300,
                    code: 1
                },
                crate::ai_events::Record {
                    timestamp: 925,
                    code: 1
                },
                crate::ai_events::Record {
                    timestamp: 1550,
                    code: 13
                },
                crate::ai_events::Record {
                    timestamp: 1550,
                    code: 1
                },
            ]
        );
        for (index, profile) in profiles[..10].iter().enumerate() {
            let queue = crate::ai_events::Timetable::from_profile(profile, 1);
            assert!(queue.count > 20 && queue.count < 50);
            let reversals = queue.records[..queue.count as usize]
                .windows(2)
                .filter(|r| r[0].timestamp > r[1].timestamp)
                .collect::<Vec<_>>();
            if index == 9 {
                // Supplied THIMBLE lines518/519 append14:30 rent before14:00
                // spoilage. Preserve script order rather than sorting it.
                assert_eq!(
                    reversals,
                    [&[
                        crate::ai_events::Record {
                            timestamp: 9050,
                            code: 1
                        },
                        crate::ai_events::Record {
                            timestamp: 8750,
                            code: 4
                        },
                    ][..]]
                );
            } else {
                assert!(reversals.is_empty());
            }
        }
    }
    #[test]
    fn executes_lua4_tables_events_and_host_results() {
        let mut vm = Runtime::new().unwrap();
        vm.register("emit", 17).unwrap();
        vm.set_number("base", 20.).unwrap();
        vm.execute("setup", b"t={}; for i=0,2 do t[i]=i*2 end; function EVENT_Apply(a) if a then total=base+t[2]+a else total=-1 end; answer=emit('rent',a,nil); end", &mut |_,_| unreachable!()).unwrap();
        let mut observed = Vec::new();
        vm.call("EVENT_Apply", &[0.], &mut |id, args| {
            observed.push((id, args.to_vec()));
            Ok(Some(37.25))
        })
        .unwrap();
        assert_eq!(vm.number("total").unwrap(), Some(24.)); // zero is true in Lua4
        assert_eq!(vm.number("answer").unwrap(), Some(37.25));
        assert_eq!(
            observed,
            [(
                17,
                vec![
                    Value::Bytes(b"rent".to_vec()),
                    Value::Number(0.),
                    Value::Nil
                ]
            )]
        );
        assert_eq!(vm.number("dofile").unwrap(), None);
        assert_eq!(vm.number("execute").unwrap(), None);
    }
    #[test]
    fn errors_and_panics_return_without_losing_vm_or_rust_cleanup() {
        let mut vm = Runtime::new().unwrap();
        vm.register("fail", 1).unwrap();
        assert!(
            vm.execute("syntax", b"function (", &mut |_, _| unreachable!())
                .is_err()
        );
        let error = vm
            .execute("host error", b"before=1; fail(); after=1", &mut |_, _| {
                Err("binding rejected".into())
            })
            .unwrap_err();
        assert!(error.contains("binding rejected"));
        assert_eq!(vm.number("before").unwrap(), Some(1.));
        assert_eq!(vm.number("after").unwrap(), None);
        struct Guard<'a>(&'a mut bool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                *self.0 = true;
            }
        }
        let mut dropped = false;
        let error = vm
            .execute("panic", b"fail()", &mut |_, _| {
                let _guard = Guard(&mut dropped);
                panic!("test callback panic");
            })
            .unwrap_err();
        assert!(dropped);
        assert!(error.contains("panicked"));
        vm.execute("recovery", b"after=2", &mut |_, _| unreachable!())
            .unwrap();
        assert_eq!(vm.number("after").unwrap(), Some(2.));
        assert!(
            vm.execute("unsupported", b"fail({})", &mut |_, _| unreachable!())
                .unwrap_err()
                .contains("requires nil")
        );
    }
}
