//! Synchronous modal dismissal shared by all live level-script invocations.
use super::Hooks;
use crate::lua4::Runtime;

pub fn register(vm: &mut Runtime) -> Result<(), String> {
    vm.register_prompt_dismiss("TutorialDismissPrompt", 725, 749)
}

pub fn handle(id: u32, hooks: &mut impl Hooks) -> Result<Option<f64>, String> {
    match id {
        725 => hooks
            .begin_prompt_dismiss()
            .map(|context| context.map(f64::from)),
        749 => {
            hooks.finish_prompt_dismiss()?;
            Ok(None)
        }
        _ => Err("unknown prompt dismissal callback".into()),
    }
}
