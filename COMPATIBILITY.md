# Compatibility modes

The native game defaults to fixing documented original blockers. Start it with
`--strict-v14` to disable intentional fixes and retain the recovered v1.4 rules
for fresh sessions. This is a policy switch, not a claim that every part of the
engine has achieved exact original behavior.

## Tutorial 5 construction rights

The original v1.4 has a [reported tutorial building-rights defect](https://www.gamepressure.com/download/monopoly-tycoon-14-patch/z3135e5).
The supplied tutorial declares building rights in its initialization script,
but the subsequent original ownership reset clears them. The tutorial then
asks the player to build while leaving the rights-activation control disabled.

The default mode preserves nonzero rights declared by the supplied Tutorial 5
script across that fresh ownership reset. It restores the declared value only
where ownership is unchanged and the reset left rights at zero. It does not
grant ownership, alter money or timers, change other levels, or edit game files.

Strict mode retains the zero rights and the resulting construction gate. It
does not undo changes already recorded in a save. Loading preserves saved
state without applying the fresh-start repair.

Tutorial 8 remains withheld pending its own verification. No fix is inferred
for it merely because its scripts resemble Tutorial 5.
