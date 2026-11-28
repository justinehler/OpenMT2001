use super::*;
use level_script::Hooks as _;
use openmt2001::level_script::prompt_dismiss;

fn message(hooks: &mut fresh_city::Hooks, context: u32) {
    hooks
        .command(
            level_script::Command::TutorialMessage,
            &[
                Value::Number(1.),
                Value::Number(2.),
                Value::Number(context as f64),
            ],
        )
        .unwrap();
}

fn runtime() -> openmt2001::lua4::Runtime {
    let mut vm = openmt2001::lua4::Runtime::new().unwrap();
    // Use the production closure and callback dispatcher, with the actual
    // worker modal queue. Only unrelated level/world bindings are omitted.
    prompt_dismiss::register(&mut vm).unwrap();
    vm.register("EnableBlockPicking", 732).unwrap();
    vm.register("DisableAll3DPicking", 721).unwrap();
    vm.register("Message", 724).unwrap();
    vm.register("HeadContext", 10000).unwrap();
    vm.execute(
        "dismiss bindings",
        b"Interface={DismissPrompt=TutorialDismissPrompt,EnableBlockPicking=EnableBlockPicking,DisableAll3DPicking=DisableAll3DPicking,CreateMessage=Message}",
        &mut |_, _| Err("unexpected setup callback".into()),
    ).unwrap();
    vm
}

fn callback(hooks: &mut fresh_city::Hooks, id: u32, args: &[Value]) -> Result<Option<f64>, String> {
    match id {
        725 | 749 => prompt_dismiss::handle(id, hooks),
        10000 => Ok(hooks.script_prompt().map(|r| r.dismissal_id as f64)),
        721 | 724 | 732 => {
            hooks.command(
                match id {
                    721 => level_script::Command::TutorialDisableAllPicking,
                    724 => level_script::Command::TutorialMessage,
                    _ => level_script::Command::TutorialEnableBlockPicking,
                },
                args,
            )?;
            Ok(None)
        }
        _ => Err(format!("unexpected callback {id}")),
    }
}

#[test]
fn synchronous_prompt_dismiss_updates_caller_and_preserves_picking_write_order() {
    for producer in ["TutorialEvent", "Picked3D"] {
        let mut vm = runtime();
        let mut hooks = fresh_city::Hooks::new();
        message(&mut hooks, 17);
        vm.execute(
            "nested dismiss",
            br"
            value=0
            function CATCHEVENT_InterfacePromptDismiss(context)
                value=context+1
                head_during=HeadContext()
                Interface.EnableBlockPicking(9)
                Interface.CreateMessage(1,2,23)
                head_after_append=HeadContext()
            end
            function Outer(a)
                Interface.DismissPrompt()
                seen=value
                head_after=HeadContext()
                Interface.DisableAll3DPicking()
            end
            CATCHEVENT_TutorialEvent=Outer
            CATCHEVENT_Picked3D=Outer
        ",
            &mut |id, args| callback(&mut hooks, id, args),
        )
        .unwrap();
        vm.call(&format!("CATCHEVENT_{producer}"), &[1.], &mut |id, args| {
            callback(&mut hooks, id, args)
        })
        .unwrap();
        assert_eq!(vm.number("seen").unwrap(), Some(18.));
        assert_eq!(vm.number("head_during").unwrap(), Some(17.));
        assert_eq!(vm.number("head_after_append").unwrap(), Some(17.));
        assert_eq!(vm.number("head_after").unwrap(), Some(23.));
        let process = process_runtime::test_fixture();
        for (command, args) in &hooks.commands {
            apply_source_picking_command(&process, command, args).unwrap();
        }
        let picking = process.picking_projection().unwrap();
        assert_eq!(picking.common, 0, "outer disable follows nested enable");
        assert_eq!(picking.block_id, 9, "nested filter write survives disable");
        assert_eq!(hooks.script_prompt_closes, [4]);
        // The presenter gets nested admissions before the old head's close.
        let mut presented = prompt::State::default();
        let requests = hooks
            .commands
            .iter()
            .filter(|(command, _)| matches!(command, level_script::Command::TutorialMessage))
            .map(|(_, args)| script_request(args, true, false).unwrap())
            .collect();
        let closes: Vec<_> = hooks
            .script_prompt_closes
            .iter()
            .map(|boundary| {
                hooks.commands[..*boundary]
                    .iter()
                    .filter(|(command, _)| {
                        matches!(command, level_script::Command::TutorialMessage)
                    })
                    .count()
            })
            .collect();
        presented.apply_requests(requests, &closes);
        assert_eq!(presented.request().unwrap().dismissal_id, 23);
    }
}

#[test]
fn synchronous_prompt_dismiss_zero_context_suppresses_catch_and_promotes_queue() {
    let mut vm = runtime();
    let mut hooks = fresh_city::Hooks::new();
    message(&mut hooks, 0);
    message(&mut hooks, 7);
    vm.execute(
        "zero context",
        br"
        called=0
        function CATCHEVENT_InterfacePromptDismiss(a) called=1 end
        Interface.DismissPrompt()
        after=HeadContext()
    ",
        &mut |id, args| callback(&mut hooks, id, args),
    )
    .unwrap();
    assert_eq!(vm.number("called").unwrap(), Some(0.));
    assert_eq!(vm.number("after").unwrap(), Some(7.));
    assert_eq!(hooks.script_prompt_closes, [3]);
}

#[test]
fn synchronous_prompt_dismiss_missing_catch_and_empty_queue_are_safe() {
    let mut vm = runtime();
    let mut hooks = fresh_city::Hooks::new();
    message(&mut hooks, 17);
    vm.execute(
        "missing catch",
        b"Interface.DismissPrompt(); Interface.DismissPrompt()",
        &mut |id, args| callback(&mut hooks, id, args),
    )
    .unwrap();
    assert!(hooks.script_prompt().is_none());
    assert_eq!(hooks.script_prompt_closes, [2]);
}

#[test]
fn synchronous_prompt_dismiss_error_unlinks_after_catch_and_keeps_vm_usable() {
    let mut vm = runtime();
    let mut hooks = fresh_city::Hooks::new();
    message(&mut hooks, 17);
    let error = vm
        .execute(
            "failed catch",
            br"
        function CATCHEVENT_InterfacePromptDismiss(a)
            head_during=HeadContext()
            MissingFunction()
        end
        Interface.DismissPrompt()
        after=1
    ",
            &mut |id, args| callback(&mut hooks, id, args),
        )
        .unwrap_err();
    assert!(!error.is_empty());
    assert_eq!(vm.number("head_during").unwrap(), Some(17.));
    assert_eq!(vm.number("after").unwrap(), None);
    assert!(hooks.script_prompt().is_none());
    assert_eq!(hooks.script_prompt_closes, [2]);
    vm.execute("reuse", b"after=2", &mut |id, args| {
        callback(&mut hooks, id, args)
    })
    .unwrap();
    assert_eq!(vm.number("after").unwrap(), Some(2.));
}
