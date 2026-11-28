//! Shared ordinary-level Camera.SetMode binding.
use super::{Command, Runtime, Value};

pub fn register(vm: &mut Runtime) -> Result<(), String> {
    // 4d9d40 reads both arguments. Retain the descriptor instead of dropping
    // it at the result-only adapter before ordinary level events can select it.
    vm.register_coerced("C_ResultCameraMode", 701, "nn")?;
    vm.alias_function("Camera", "SetMode", "C_ResultCameraMode")
}

pub fn command(arguments: &[Value]) -> Result<(Command, &[Value]), String> {
    let [Value::Number(mode), Value::Number(_)] = arguments else {
        return Err("Camera.SetMode requires its coerced mode and target".into());
    };
    if crate::economy::ftol_low_word(*mode) == 12 {
        // The presentation owner already sends this through ordinary block
        // selection, including the selected block and its information panel.
        Ok((Command::TutorialCameraMode, arguments))
    } else {
        // Preserve the result-sequence adapter's one-argument contract.
        Ok((Command::ResultCameraMode, &arguments[..1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_level_mode_keeps_block_target_and_result_contract() {
        let mut vm = Runtime::new().unwrap();
        vm.execute("Camera table", b"Camera={}", &mut |_, _| unreachable!())
            .unwrap();
        register(&mut vm).unwrap();
        vm.execute(
            "ordinary level event",
            b"function EVENT_Level() Camera.SetMode(12, 7) end",
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut calls = Vec::new();
        let mut handler = |id, args: &[Value]| {
            assert_eq!(id, 701);
            let (command, args) = command(args)?;
            calls.push((command, args.to_vec()));
            Ok(None)
        };
        vm.call("EVENT_Level", &[0.], &mut handler).unwrap();
        vm.execute("result camera", b"Camera.SetMode(3)", &mut handler)
            .unwrap();
        assert!(matches!(calls[0].0, Command::TutorialCameraMode));
        assert_eq!(calls[0].1, [Value::Number(12.), Value::Number(7.)]);
        assert!(matches!(calls[1].0, Command::ResultCameraMode));
        assert_eq!(calls[1].1, [Value::Number(3.)]);
    }
}
