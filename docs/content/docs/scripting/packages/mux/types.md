---
title: "mux value types"
type: docs
---

Record shapes and aliases used by the callable signatures in this reference.

## ConfigValue

Alias: `string|number|boolean Scalar value returned by the live configuration registry.`

## DbRef

Alias: `integer Database object reference.`

## NativeErrorRoot

Alias: `"mux"|"btech"|"testing" Root of a checked native error-code tree.`

## StateValue

Alias: `string|boolean|number Scalar value supported by persistent object state.`

## TelnetEnvironmentKind

Alias: `"var"|"uservar" RFC 1572 NEW-ENVIRON variable namespace.`

## CreateObjectOptions

Alias: `CreateRoomOptions|CreateThingOptions|CreateExitOptions`

## ChannelEmitOptions

- `no_header`: `boolean` — Send the message without the usual `[channel]` prefix.

## ChannelWhoOptions

- `all`: `boolean` — Include inactive membership records.

## ChannelMember

- `object`: `Object` — Live member object.
- `listening`: `boolean` — Whether the member is currently listening to the channel.

## CaughtError

- `code`: `string` — Stable dotted error code.
- `traceback`: `string` — Traceback captured by `mux.error.pcall`.
- `message`: `string` — Optional human-readable failure description.
- `detail`: `any` — Optional structured context.
- `cause`: `any` — Optional earlier failure.

## ErrorCode

- `code`: `string` — Fully qualified error code represented by this node.

## ErrorFields

- `code`: `string|ErrorCode` — Stable dotted code or checked code node.
- `message`: `string` — Human-readable failure description.
- `detail`: `any` — Optional structured context.
- `cause`: `any` — Optional earlier failure.

## ErrorCodeTree

- `[string]`: `ErrorCodeTree` — Checked child code segment.

## Error

- `code`: `string` — Stable dotted error code.
- `message`: `string` — Human-readable failure description.
- `detail`: `any` — Optional structured context.
- `cause`: `any` — Earlier failure preserved by wrapping.
- `traceback`: `string` — Traceback added by `mux.error.pcall`.

## Connection

- `object`: `Object` — Connected player.
- `name`: `string` — Current object name.
- `connected_for`: `integer` — Connected duration in seconds.
- `idle_for`: `integer` — Idle duration in seconds.

## WhoSummary

- `hidden`: `integer` — Hidden-player count; currently always zero for this non-privileged view.
- `record`: `integer` — Record simultaneous-player count.
- `maximum`: `integer` — Configured limit, or nil when unlimited.

## StateEntry

- `key`: `string` — Stored state key.
- `value`: `StateValue` — Stored scalar value.

## StyleOptions

- `foreground`: `string` — Palette foreground name.
- `background`: `string` — Palette background name.
- `bold`: `boolean` — Whether to enable bold intensity.
- `underline`: `boolean` — Whether to underline the text.
- `inverse`: `boolean` — Whether to swap foreground and background presentation.

## CreateRoomOptions

- `type`: `RoomObjectType` — The current runtime's `mux.world.types.ROOM` constant.
- `name`: `string` — Required UTF-8 name, optionally containing valid styled-text markup.
- `zone`: `DbRef|Object` — Live thing or room to assign; omission preserves the native creator's inherited zone.

## CreateThingOptions

- `type`: `ThingObjectType` — The current runtime's `mux.world.types.THING` constant.
- `name`: `string` — Required UTF-8 name, optionally containing valid styled-text markup.
- `location`: `DbRef|Object` — Required object that can contain the new thing.
- `home`: `DbRef|Object` — Home object; defaults to `location` when omitted.
- `zone`: `DbRef|Object` — Live thing or room to assign; omission preserves the native creator's inherited zone.

## CreateExitOptions

- `type`: `ExitObjectType` — The current runtime's `mux.world.types.EXIT` constant.
- `name`: `string` — Required UTF-8 name, optionally containing valid styled-text markup.
- `location`: `DbRef|Object` — Required source object capable of holding exits.
- `destination`: `DbRef|Object` — Optional destination capable of containing objects; omission leaves the exit unlinked.
- `zone`: `DbRef|Object` — Live thing or room to assign; omission preserves the native creator's inherited zone.

## TeleportOptions

- `object`: `DbRef|Object` — Required thing or player to move.
- `destination`: `DbRef|Object` — Required object capable of containing objects.

## DestroyOptions

- `override`: `boolean` — Whether to bypass the target's SAFE flag; core objects and Wizard players remain protected.

## LockPassesOptions

- `object`: `DbRef|Object` — Required object whose lock is tested.
- `enactor`: `DbRef|Object` — Required object attempting the action.
- `lock`: `Lock` — Required typed lock constant from `mux.world.locks`.
- `cause`: `DbRef|Object` — Object that caused the action; defaults to `enactor`.
- `subject`: `DbRef|Object` — Object acted upon in the lock context; defaults to `enactor`.

## ContentsOptions

- `types`: `ObjectType[]` — Native object kinds to include; an empty array matches nothing.
- `visible_to`: `DbRef|Object` — Viewer whose native visibility rules are applied.

## ListObjectsOptions

- `types`: `ObjectType[]` — Native object kinds to include; an empty array matches nothing.
- `in_zone`: `DbRef|Object` — Include only objects directly assigned to this zone.

## ChannelFlagNamespace

- `PUBLIC`: `ChannelFlag` — Makes the channel visible without a successful join lock.
- `LOUD`: `ChannelFlag` — Announces applicable connection and presence changes.
- `TRANSPARENT`: `ChannelFlag` — Relaxes hidden-member filtering in native channel displays.

## MuxArgInvalidErrorCode

- `code`: `"mux.arg.invalid"`

## MuxCheckingUnavailableErrorCode

- `code`: `"mux.unavailable.checking"`

## MuxRuntimeErrorCode

- `code`: `"mux.runtime"`

## MuxStateInvalidErrorCode

- `code`: `"mux.state.invalid"`

## MuxStateValueTooLargeErrorCode

- `code`: `"mux.state.value_too_large"`

## MuxStateUnavailableErrorCode

- `code`: `"mux.state.unavailable"`

## MuxObjectInvalidErrorCode

- `code`: `"mux.object.invalid"`

## MuxObjectUnavailableErrorCode

- `code`: `"mux.object.unavailable"`

## MuxFlagInvalidErrorCode

- `code`: `"mux.flag.invalid"`

## MuxPowerInvalidErrorCode

- `code`: `"mux.power.invalid"`

## MuxAccessInvalidErrorCode

- `code`: `"mux.access.invalid"`

## MuxConnectionInvalidErrorCode

- `code`: `"mux.connection.invalid"`

## MuxConnectionUnavailableErrorCode

- `code`: `"mux.connection.unavailable"`

## MuxChannelInvalidErrorCode

- `code`: `"mux.channel.invalid"`

## MuxChannelFlagInvalidErrorCode

- `code`: `"mux.channel_flag.invalid"`

## MuxTextInvalidErrorCode

- `code`: `"mux.text.invalid"`

## MuxModuleInvalidErrorCode

- `code`: `"mux.module.invalid"`

## MuxModuleUnavailableErrorCode

- `code`: `"mux.module.unavailable"`

## MuxConfigNotFoundErrorCode

- `code`: `"mux.config.not_found"`

## MuxConfigUnsupportedErrorCode

- `code`: `"mux.config.unsupported"`

## MuxInternalErrorCode

- `code`: `"mux.internal"`

## MuxArgErrorCodes

- `invalid`: `MuxArgInvalidErrorCode` — `mux.arg.invalid`.

## MuxUnavailableErrorCodes

- `checking`: `MuxCheckingUnavailableErrorCode` — `mux.unavailable.checking`.

## MuxStateErrorCodes

- `invalid`: `MuxStateInvalidErrorCode` — `mux.state.invalid`.
- `value_too_large`: `MuxStateValueTooLargeErrorCode` — `mux.state.value_too_large`.
- `unavailable`: `MuxStateUnavailableErrorCode` — `mux.state.unavailable`.

## MuxObjectErrorCodes

- `invalid`: `MuxObjectInvalidErrorCode` — `mux.object.invalid`.
- `unavailable`: `MuxObjectUnavailableErrorCode` — `mux.object.unavailable`.

## MuxFlagErrorCodes

- `invalid`: `MuxFlagInvalidErrorCode` — `mux.flag.invalid`.

## MuxPowerErrorCodes

- `invalid`: `MuxPowerInvalidErrorCode` — `mux.power.invalid`.

## MuxAccessErrorCodes

- `invalid`: `MuxAccessInvalidErrorCode` — `mux.access.invalid`.

## MuxConnectionErrorCodes

- `invalid`: `MuxConnectionInvalidErrorCode` — `mux.connection.invalid`.
- `unavailable`: `MuxConnectionUnavailableErrorCode` — `mux.connection.unavailable`.

## MuxChannelErrorCodes

- `invalid`: `MuxChannelInvalidErrorCode` — `mux.channel.invalid`.

## MuxChannelFlagErrorCodes

- `invalid`: `MuxChannelFlagInvalidErrorCode` — `mux.channel_flag.invalid`.

## MuxTextErrorCodes

- `invalid`: `MuxTextInvalidErrorCode` — `mux.text.invalid`.

## MuxModuleErrorCodes

- `invalid`: `MuxModuleInvalidErrorCode` — `mux.module.invalid`.
- `unavailable`: `MuxModuleUnavailableErrorCode` — `mux.module.unavailable`.

## MuxConfigErrorCodes

- `not_found`: `MuxConfigNotFoundErrorCode` — `mux.config.not_found`.
- `unsupported`: `MuxConfigUnsupportedErrorCode` — `mux.config.unsupported`.

## MuxMacroInvalidErrorCode

- `code`: `"mux.macro.invalid"`

## MuxMacroNotFoundErrorCode

- `code`: `"mux.macro.not_found"`

## MuxMacroExistsErrorCode

- `code`: `"mux.macro.exists"`

## MuxMacroSlotsFullErrorCode

- `code`: `"mux.macro.slots_full"`

## MuxMacroErrorCodes

- `invalid`: `MuxMacroInvalidErrorCode` — `mux.macro.invalid`.
- `not_found`: `MuxMacroNotFoundErrorCode` — `mux.macro.not_found`.
- `exists`: `MuxMacroExistsErrorCode` — `mux.macro.exists`.
- `slots_full`: `MuxMacroSlotsFullErrorCode` — `mux.macro.slots_full`.

## MuxErrorCodes

- `arg`: `MuxArgErrorCodes` — Invalid-argument code branch.
- `macro`: `MuxMacroErrorCodes` — Player-macro code branch.
- `unavailable`: `MuxUnavailableErrorCodes` — Runtime-availability code branch.
- `runtime`: `MuxRuntimeErrorCode` — `mux.runtime`.
- `state`: `MuxStateErrorCodes` — Persistent-state code branch.
- `object`: `MuxObjectErrorCodes` — Database-object code branch.
- `flag`: `MuxFlagErrorCodes` — Object-flag code branch.
- `power`: `MuxPowerErrorCodes` — Object-power code branch.
- `access`: `MuxAccessErrorCodes` — Command-access code branch.
- `connection`: `MuxConnectionErrorCodes` — Connection code branch.
- `channel`: `MuxChannelErrorCodes` — Communication-channel code branch.
- `channel_flag`: `MuxChannelFlagErrorCodes` — Channel-flag code branch.
- `text`: `MuxTextErrorCodes` — Styled-text code branch.
- `module`: `MuxModuleErrorCodes` — Lua-module code branch.
- `config`: `MuxConfigErrorCodes` — Configuration code branch.
- `internal`: `MuxInternalErrorCode` — `mux.internal`.

## TestingAssertionErrorCode

- `code`: `"testing.assertion"`

## TestingRuntimeErrorCode

- `code`: `"testing.runtime"`

## TestingErrorCodes

- `assertion`: `TestingAssertionErrorCode` — `testing.assertion`.
- `runtime`: `TestingRuntimeErrorCode` — `testing.runtime`.

## BtechPartNotFoundErrorCode

- `code`: `"btech.part.not_found"`

## BtechPartAmbiguousErrorCode

- `code`: `"btech.part.ambiguous"`

## BtechPartWrongKindErrorCode

- `code`: `"btech.part.wrong_kind"`

## BtechTemplateNotFoundErrorCode

- `code`: `"btech.template.not_found"`

## BtechTemplateInvalidErrorCode

- `code`: `"btech.template.invalid"`

## BtechOperationFailedErrorCode

- `code`: `"btech.operation.failed"`

## BtechPartErrorCodes

- `not_found`: `BtechPartNotFoundErrorCode` — `btech.part.not_found`.
- `ambiguous`: `BtechPartAmbiguousErrorCode` — `btech.part.ambiguous`.
- `wrong_kind`: `BtechPartWrongKindErrorCode` — `btech.part.wrong_kind`.

## BtechTemplateErrorCodes

- `not_found`: `BtechTemplateNotFoundErrorCode` — `btech.template.not_found`.
- `invalid`: `BtechTemplateInvalidErrorCode` — `btech.template.invalid`.

## BtechOperationErrorCodes

- `failed`: `BtechOperationFailedErrorCode` — `btech.operation.failed`.

## BtechErrorCodes

- `part`: `BtechPartErrorCodes` — Inventory-part code branch.
- `template`: `BtechTemplateErrorCodes` — Unit-template code branch.
- `operation`: `BtechOperationErrorCodes` — Gameplay-operation code branch.

## AccessNamespace

- `PUBLIC`: `Access` — Allows every invoker; also the default when access is omitted.
- `WIZARD`: `Access` — Allows Wizards and God.
- `GOD`: `Access` — Allows only God.

## FlagNamespace

- `ANSI`: `Flag` — Enables ANSI-capable output for the object.
- `AUDIBLE`: `Flag` — Enables sound-capable notifications associated with the object.
- `AUDITORIUM`: `Flag` — Applies auditorium-style speech propagation.
- `BLIND`: `Flag` — Marks the object as unable to see normally.
- `CONNECTED`: `Flag` — Marks a player as currently connected.
- `DARK`: `Flag` — Hides the object according to native visibility rules.
- `FLOATING`: `Flag` — Prevents ordinary location inheritance for the object.
- `GAGGED`: `Flag` — Prevents the object from speaking normally.
- `GOING`: `Flag` — Marks the object for deferred destruction.
- `HALTED`: `Flag` — Prevents queued command execution by the object.
- `IN_CHARACTER`: `Flag` — Marks the object as participating in in-character play.
- `LIGHT`: `Flag` — Makes the object visible through native light rules.
- `MONITOR`: `Flag` — Enables command monitoring behavior.
- `NO_COMMAND`: `Flag` — Excludes commands stored on the object from command matching.
- `SAFE`: `Flag` — Protects the object from ordinary destruction.
- `SUSPECT`: `Flag` — Marks a player for suspect-activity monitoring.
- `TRANSPARENT`: `Flag` — Allows visibility through the object.
- `WIZARD`: `Flag` — Grants Wizard status under native privilege rules.
- `ZOMBIE`: `Flag` — Allows a thing to act through its owner under native rules.

## LockNamespace

- `MATCH`: `Lock` — Prefer an object that passes during key-aware matching.
- `TRAVERSE`: `Lock` — Traverse an exit.
- `TAKE`: `Lock` — Take an object.
- `USE`: `Lock` — Use an object.
- `DROP`: `Lock` — Drop an object.
- `GIVE`: `Lock` — Give an object.
- `RECEIVE`: `Lock` — Receive a given object.
- `ENTER`: `Lock` — Enter an object, room, BattleTech unit, bay, or hangar.
- `LEAVE`: `Lock` — Leave an object or room.
- `TELEPORT`: `Lock` — Teleport into a destination.
- `TELEPORT_OUT`: `Lock` — Teleport out of an origin.
- `LINK`: `Lock` — Link an exit or object.
- `SET_HOME`: `Lock` — Set an object's home to a destination.
- `SPEAK`: `Lock` — Speak in a location.
- `CHANNEL_JOIN`: `Lock` — Join a channel.
- `CHANNEL_TRANSMIT`: `Lock` — Transmit on a channel.
- `CHANNEL_RECEIVE`: `Lock` — Receive channel traffic.
- `IDENTIFY_BUILDING`: `Lock` — Silently identify a visible BattleTech structure.

## PowerNamespace

- `IDLE`: `Power` — Exempts a player from ordinary idle-timeout handling.

## ObjectTypeNamespace

- `ROOM`: `RoomObjectType` — Detached room kind accepted by `mux.world.create_object`.
- `THING`: `ThingObjectType` — Contained thing kind accepted by `mux.world.create_object`.
- `EXIT`: `ExitObjectType` — Attached exit kind accepted by `mux.world.create_object`.
- `PLAYER`: `PlayerObjectType` — Player kind; existing players may have this type, but scripts cannot create them.

## MuxPackage

- `macro`: `MuxMacroPackage` — Trusted macro administration.
- `comsys`: `MuxComsysPackage` — Trusted live communication-channel administration.
- `config`: `MuxConfigPackage` — Read-only scalar server configuration.
- `error`: `MuxErrorPackage` — Structured errors and checked code nodes.
- `session`: `MuxSessionPackage` — Live connections and interactive flows.
- `telnet`: `MuxTelnetPackage` — Telnet protocol state and capabilities.
- `text`: `MuxTextPackage` — Styled-text utilities.
- `world`: `MuxWorldPackage` — World database object access.

## MuxComsysPackage

- `flags`: `ChannelFlagNamespace` — Immutable typed channel-flag constants.

## MuxErrorPackage

- `codes`: `MuxErrorCodes` — Checked native `mux` code tree.

## MuxWorldPackage

- `access`: `AccessNamespace` — Immutable namespace of command-access constants.
- `flags`: `FlagNamespace` — Immutable namespace of registered flag constants.
- `locks`: `LockNamespace` — Immutable namespace of native lock constants.
- `powers`: `PowerNamespace` — Immutable namespace of registered power constants.
- `types`: `ObjectTypeNamespace` — Immutable namespace of native object-kind constants.

## MacroFlagConstants

- `LOCKED`: `MacroFlag` — Blocks player definition edits and renaming, including by the owner.
- `READ`: `MacroFlag` — Allows other players to discover, inspect, and attach the set.
- `WRITE`: `MacroFlag` — Allows other players to edit the unlocked set.

## MacroEntry

- `alias`: `string`
- `expansion`: `string`

## MacroAttachment

- `slot`: `integer` — Zero-based slot, 0 through 4.
- `set`: `MacroSet`
- `selected`: `boolean`

## MuxMacroPackage

- `flags`: `MacroFlagConstants`

## MuxCommandDefinition

- `name`: `string` — Canonical command token, without whitespace or slash.
- `permission`: `'everyone'|'wizard'|'god'` — Required actor authority; restricted entries are skipped.
- `pattern`: `string` — Lua string.find pattern.
- `handler`: `fun(ctx: table, ...):` — boolean? True consumes the input; false/nil continues dispatch.
