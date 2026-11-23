# Compatibility modes

The native game defaults to fixing documented original blockers. Start it with
`--strict-v14` to disable intentional fixes and retain the recovered v1.4 rules
where documented below. This is a policy switch, not a claim that every part of the
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

## Tutorial 8 startup rights

A connected original-input engine check found the same startup loss of the
rights declared by Tutorial 8's supplied script. Modern mode preserves those
declared rights using the same fresh-start, unchanged-owner predicates.
Strict mode and saved loads retain their previous behavior.

This is a documented compatibility choice based on the measured engine path;
it does not establish that the original executable has a Tutorial 8 defect.
Its connected check constructs the authored bakery, grocery store and cafe,
then verifies completion and the next unlock through profile reload.

## Imported resident models

An original save imported by this engine can retain a freshly randomized
resident model while restoring the saved resident's age. A loaded city was
observed requesting frame 173 from teenage geometry containing 169 frames.
The same preserved save records resident slot 315 as an older person.
This is evidence about the engine's import path; it does not establish that
the original executable crashes on that save.

Modern mode binds imported residents to models selected from their saved age
and pool identity, and updates the associated model frame counts. The repair
runs at the idle load boundary, validates the retained frame, and preserves
resident records, pose, frame, clocks and random streams. It does not clamp
animation frames. Strict mode retains the recovered low-level load behavior.

Saves with a modern runtime extension retain their explicitly serialized
model and controller state instead of applying this import repair.

## Ambient tree lighting

The recovered renderer transforms sunlight by each tree's scaled basis before
indexing a 256-entry lighting palette. A supplied-save engine test reached an
out-of-range index during the afternoon daylight update. The original lookup
has no bounds check; this does not establish its visible result or a crash in
the original executable.

Modern mode saturates the trunk's palette index to the nearest valid endpoint,
keeping valid indices unchanged. Strict mode keeps the checked low-level
lookup and reports an error for this unsupported out-of-table case. It does
not emulate arbitrary adjacent original memory. Cached original normals and
textures, foliage lighting, placement and simulation remain unchanged.

## Offline Skirmish maps

The map picker follows the supplied v1.4 executable's separate conditions and
map lists. One preserved recording shows a different full-height conditions
layout; its exact executable/resource pairing is unresolved, so equivalence to
that recording is not claimed.

Map names come from `scripts/MAPS/multiplayer/`, while fresh startup reads the
matching `scripts/MAPS/` file. The engine uses a deterministic portable sort;
the original used Windows filesystem enumeration order. Missing, ambiguous,
unsafe or oversized inputs are refused, and an empty catalog disables OK.
These host input boundaries apply in both compatibility modes.

Selected-layout saves retain map provenance in an engine-specific version 6
runtime extension. Existing version 5 standard-layout saves remain loadable.
An imported materialized city whose source map is unknown retains that unknown
identity instead of being labeled as the empty layout. Loading restores the
saved city and does not execute the selected map script again. These extensions
do not establish save compatibility with the original executable.
