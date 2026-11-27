//! Protected-operation tests. Bare Runtime intentionally
//! has no base-library rawget/rawset functions; aliases use existing closures.
use super::*;

#[test]
fn protected_alias_keeps_closure_identity_and_tagged_property_without_base_library() {
    let mut vm = Runtime::new().unwrap();
    vm.register_number_properties("Game", 747).unwrap();
    vm.register_coerced("C_ResultEnd", 707, "nn").unwrap();
    vm.alias_function("Game", "End", "C_ResultEnd").unwrap();
    let mut performance = 2.;
    let mut end = None;
    vm.execute("protected alias",b"no_rawset=(rawset==nil); no_rawget=(rawget==nil); same=(Game.End==C_ResultEnd); Game.performance=-3; observed=Game.performance; Game.End('1.5',2)",&mut |id,args|{
        match id {
            747=>{assert_eq!(args,&[Value::Bytes(b"performance".to_vec())]);Ok(Some(performance))},
            748=>{assert_eq!(args.first(),Some(&Value::Bytes(b"performance".to_vec())));
                let Value::Number(n)=args[1] else {panic!("numeric property not C-coerced")};performance=n;Ok(None)},
            707=>{end=Some(args.to_vec());Ok(None)},
            _=>panic!("unexpected callback {id}"),
        }
    }).unwrap();
    assert_eq!(vm.number("no_rawset").unwrap(), Some(1.));
    assert_eq!(vm.number("no_rawget").unwrap(), Some(1.));
    assert_eq!(vm.number("same").unwrap(), Some(1.));
    assert_eq!(vm.number("observed").unwrap(), Some(-3.));
    assert_eq!(end, Some(vec![Value::Number(1.5), Value::Number(2.)]));
}

#[test]
fn failed_alias_validation_keeps_previous_method_and_vm_serviceable() {
    let mut vm = Runtime::new().unwrap();
    vm.register_number_properties("Game", 747).unwrap();
    vm.register("C_End", 707).unwrap();
    vm.alias_function("Game", "End", "C_End").unwrap();
    vm.set_number("not_a_function", 9.).unwrap();
    vm.set_number("not_a_table", 7.).unwrap();
    assert!(
        vm.alias_function("missing_table", "End", "C_End")
            .unwrap_err()
            .contains("not a table")
    );
    assert!(
        vm.alias_function("not_a_table", "End", "C_End")
            .unwrap_err()
            .contains("not a table")
    );
    assert!(
        vm.alias_function("Game", "End", "missing_function")
            .unwrap_err()
            .contains("not a function")
    );
    assert!(
        vm.alias_function("Game", "End", "not_a_function")
            .unwrap_err()
            .contains("not a function")
    );
    assert!(vm.alias_function("Game\0", "End", "C_End").is_err());
    let mut calls = 0;
    vm.execute(
        "after rejected aliases",
        b"same=(Game.End==C_End); Game.End()",
        &mut |id, args| {
            assert_eq!(id, 707);
            assert!(args.is_empty());
            calls += 1;
            Ok(None)
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(vm.number("same").unwrap(), Some(1.));
}
