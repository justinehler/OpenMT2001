//! Regressions. Explicit process-scoped stand-in for core-only Lua
//! tests; production's real ProcessRuntime identity is checked in frontend.
use super::*;
use std::{cell::Cell,rc::Rc};
struct Host {word:Rc<Cell<i32>>}
impl crate::business_pool::BusinessPoolHooks for Host {
    fn owner_transfer(&mut self,_:usize,_:&mut [u8;0x220],_:u32)->Result<(),String>{Err("not admitted by performance fixture".into())}
    fn notify(&mut self)->Result<(),String>{Err("not admitted by performance fixture".into())}
}
impl crate::map_script::Hooks for Host {
    fn building_appearance(&mut self,_:usize,_:&mut [u8;0x394])->Result<(),String>{Err("not admitted by performance fixture".into())}
    fn building_created(&mut self,_:usize)->Result<(),String>{Err("not admitted by performance fixture".into())}
    fn binding_call(&mut self,_:crate::business_binding::Call,_:&mut crate::business_binding::World<'_>)->Result<(),String>{Err("not admitted by performance fixture".into())}
    fn stock_charge(&mut self,_:&mut [u8;0x220],_:f32)->Result<(),String>{Err("not admitted by performance fixture".into())}
}
impl Hooks for Host {
    fn performance8b26cc(&mut self)->Result<i32,String>{Ok(self.word.get())}
    fn set_performance8b26cc(&mut self,value:i32)->Result<(),String>{self.word.set(value);Ok(())}
    fn pause_timers(&mut self,_:crate::pause::Timers)->Result<(),String>{Err("not admitted by performance fixture".into())}
    fn calendar_timers(&mut self)->(&mut crate::timers::CalendarFrequency,&mut [crate::timers::Task]){panic!("not admitted by performance fixture")}
    fn command(&mut self,_:Command,_:&[Value])->Result<(),String>{Err("not admitted by performance fixture".into())}
}
#[test]
fn real_lua_setter_coercion_is_signed_unclamped_and_repeated(){
    let word=Rc::new(Cell::new(2));let mut hooks=Host{word:word.clone()};
    let mut vm=Runtime::new().unwrap();register_game_performance(&mut vm).unwrap();
    // Expected conversions come from 4d6980 tonumber -> _ftol/EAX, not a cast
    // or quality clamp copied out of the candidate's implementation.
    for (source,expected) in [("PERFORM_LOW",0),("1.9",1),("PERFORM_MEDIUM",1),("PERFORM_HIGH",2),("-9.9",-9),
        ("'12.7'",12),("'bad'",0),("nil",0),("{}",0),("function() end",0),
        ("4294967301",5),("2147483648",i32::MIN),("1e20",0)] {
        vm.execute("performance setter",format!("Game.performance={source}; observed=Game.performance").as_bytes(),
            &mut |id,args|game_performance_property(id,args,&mut hooks)).unwrap();
        assert_eq!(word.get(),expected,"input {source}");
        assert_eq!(vm.number("observed").unwrap(),Some(f64::from(expected)));
    }
    assert!(vm.execute("unknown property",b"Game.unrecovered=2",&mut |id,args|game_performance_property(id,args,&mut hooks)).is_err());
    assert!(vm.execute("partial failure",b"Game.performance=1; absent_function()",&mut |id,args|game_performance_property(id,args,&mut hooks)).is_err());
    assert_eq!(word.get(),1,"the setter is synchronous before a later Lua failure");
}
#[test]
fn rebind_and_nested_script_share_the_same_word_without_raw_snapshot(){
    let word=Rc::new(Cell::new(1));let mut a=Host{word:word.clone()};let mut b=Host{word:word.clone()};
    let mut first=Runtime::new().unwrap();register_game_performance(&mut first).unwrap();
    first.register_script_loader("C_RunScript").unwrap();
    first.script_buffer("inner.lua",b"Game.performance='2.9'; inner=Game.performance").unwrap();
    first.execute("nested",b"Game.performance=0; before=Game.performance; C_RunScript('inner.lua'); after=Game.performance",
        &mut |id,args|game_performance_property(id,args,&mut a)).unwrap();
    assert_eq!(first.number("before").unwrap(),Some(0.));assert_eq!(first.number("after").unwrap(),Some(2.));
    assert_eq!(first.number("inner").unwrap(),Some(2.));
    let mut second=Runtime::new().unwrap();register_game_performance(&mut second).unwrap();
    second.execute("second VM",b"extant=Game.performance; Game.performance=-3; changed=Game.performance",
        &mut |id,args|game_performance_property(id,args,&mut b)).unwrap();
    assert_eq!(second.number("extant").unwrap(),Some(2.));assert_eq!(word.get(),-3);
    first.execute("original VM reader",b"latest=Game.performance",&mut |id,args|game_performance_property(id,args,&mut a)).unwrap();
    assert_eq!(first.number("latest").unwrap(),Some(-3.));
    word.set(17);
    first.execute("external owner write",b"fresh=Game.performance",&mut |id,args|game_performance_property(id,args,&mut a)).unwrap();
    assert_eq!(first.number("fresh").unwrap(),Some(17.));
}
