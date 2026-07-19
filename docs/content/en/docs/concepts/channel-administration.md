---
title: Channel administration
linkTitle: Channel administration
type: docs
weight: 30
---

Wizards administer communication channels with `@chan`. Running it without a
switch prints the available operations. Channels do not have individual
owners, and mortals cannot invoke `@chan`. A game that wants to offer mortal
self-management must provide Lua commands that enforce its desired policy and
call the appropriate channel functionality.

## Create and inspect channels

Create a channel, make it public if desired, and inspect the result:

```text
@chan/create Staff
@chan/flags Staff=public
@chan/status Staff
@chan/status/full Staff
```

## List and inspect channels

Use `@chan/list` for the normal description view. The
`@chan/list/full` and `@chan/status/full` forms expose the capability flags,
attached channel-object dbref, membership count, and message count.

## Destroy channels

Destroy a channel with:

```text
@chan/destroy Staff
```

Destruction removes the channel and its membership records immediately.

## Manage channel membership

Inspect membership and listening state with `@chan/who Staff`. Add `/all` to
the channel argument to include inactive membership records:

```text
@chan/who Staff/all
```

A Wizard can remove a member while also on the channel:

```text
@chan/boot Staff=TroublesomePlayer
```

## Channel flags

Channel flags may be set and unset using the `@chan/flags` command.

New channels are private. Setting the `public` flag includes a channel in 
public `comlist` listings. Clear the flag with `!public` to take it private:

```text
@chan/flags Staff=!public
```

A `loud` channel announces member connections and disconnections. Removing the
`loud` flag on a channel suppresses those notices:

```text
@chan/flags Staff=loud
@chan/flags Staff=!loud
```

A `transparent` channel reveals eligible hidden members in membership output;
clearing the flag hides them:

```text
@chan/flags Staff=transparent
@chan/flags Staff=!transparent
```

## Channel member flags

Channels track join, receive, and transmit grants separately for players and
objects. Use `/pflags` for players and `/oflags` for objects:

```text
@chan/pflags Staff=join
@chan/pflags Staff=!transmit
@chan/oflags Staff=receive
@chan/oflags Staff=!join
```

An enabled flag grants that capability to every member of that type. Prefix
the capability with `!` to clear the grant. Clearing a flag does not
necessarily deny access: an attached channel object's corresponding lock can
still grant it.

## Attach an object for descriptions and locks

A channel object supplies the description displayed by normal list and status
output and can grant channel capabilities through locks. Create and describe a
dedicated object, set its locks, and attach it:

```text
@create Staff Channel Object
@desc #123=Coordination channel for game staff.
@lock/defaultlock #123=<join lock expression>
@lock/uselock #123=<transmit lock expression>
@lock/enterlock #123=<receive lock expression>
@chan/object Staff=#123
```

The lock mapping is:

| Channel capability | Channel-object lock |
| --- | --- |
| Join | Default lock |
| Transmit | Use lock |
| Receive | Enter lock |

Capability flags are permissive grants and take precedence over the need to
pass a lock. For example, if the player join flag is enabled, every player can
join regardless of the default lock. To make a lock authoritative for a type
and capability, clear that grant first:

```text
@chan/pflags Staff=!join
@chan/oflags Staff=!join
```

Repeat that pattern for `receive` or `transmit` when their locks should decide
access. Wizards bypass these access checks.

Detach the channel object with an empty value:

```text
@chan/object Staff=
```

Detaching removes the description and lock-based grants but does not change
the channel's player or object capability flags.

## Emit to channels

Use the `@chan/emit` command to send a message to every channel member:

```text
@chan/emit Staff=The meeting starts in five minutes.
```

To send a message without the channel's name prefixed before the message, use
the `/noheader` emit switch:

```text
@chan/emit/noheader Staff=Exact text sent to listeners
```
