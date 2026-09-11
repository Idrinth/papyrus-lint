# Examples

The [Simple Example](../README.md#simple-example) in the README shows one
way Papyrus Lint catches a bug `PapyrusCompiler.exe` compiles without
complaint. The examples below show more of the same kind — real bugs,
not style nitpicks — pulled from the Reliability, Bugprone, and
Performance categories of the [Implemented Lints
table](../README.md#implemented-lints). Every snippet compiles cleanly;
the linter is what actually catches the problem.

## Dereferencing a `None` Form

```papyrus
ScriptName Example extends ObjectReference

Function GreetOwner(Actor akOwner = None)
    Actor a = None
    a.SendAnimationEvent("wave")
EndFunction
```

`a` is declared `None` and never assigned anything else before it's used,
so `a.SendAnimationEvent(...)` crashes the script the moment this
function runs. The **None used as an existing Form** lint (`none-form-usage`)
flags the access; the likely fix is to assign `a` before using it, or to
guard the call:

```papyrus
ScriptName Example extends ObjectReference

Function GreetOwner(Actor akOwner = None)
    Actor a = akOwner
    If a != None
        a.SendAnimationEvent("wave")
    EndIf
EndFunction
```

## Division by zero

```papyrus
ScriptName Example

Function GetAverage(Int total, Int[] values)
    Int average = total / (values.Length - values.Length)
EndFunction
```

`values.Length - values.Length` always folds to `0`, so this division
always crashes the script at runtime — the compiler has no reason to
reject it, since dividing by zero is only ever a problem once the script
actually runs. The **Division by zero** lint (`division-by-zero`) flags
any divisor that folds to a compile-time-constant zero.

## Array index out of bounds

```papyrus
ScriptName Example

Function ResetSlots()
    Int[] slots = new Int[3]
    slots[3] = 0
EndFunction
```

`slots` was created with 3 elements, so valid indexes are `0` through
`2` — `slots[3]` is out of range. Papyrus doesn't raise a catchable
error for this; it silently no-ops the write, so `slots[3] = 0` quietly
does nothing at runtime instead of failing loudly. The **Array bounds**
lint (`array-bounds`) tracks the declared size of `new`-created arrays
and flags a literal index that falls outside it.

## Wrong return type

```papyrus
ScriptName Example

Bool Function HasEnoughGold(Actor akActor, Int amount)
    Return akActor.GetItemCount(Gold001)
EndFunction
```

`GetItemCount()` returns an `Int`, but `HasEnoughGold` is declared to
return a `Bool` — Papyrus implicitly converts the count to a boolean
(`0` becomes `False`, anything else becomes `True`), which almost
certainly isn't the actual comparison the author meant to make. The
**Return type check** lint (`return-types`) flags the mismatched
`Return`; the likely fix is an explicit comparison:

```papyrus
ScriptName Example

Bool Function HasEnoughGold(Actor akActor, Int amount)
    Return akActor.GetItemCount(Gold001) >= amount
EndFunction
```

## Integer division silently truncating

```papyrus
ScriptName Example

Function ApplyDiscount(Int price, Int discountPercent)
    Float remainingFraction = (100 - discountPercent) / 100
    Debug.Notification("You pay " + (price * remainingFraction))
EndFunction
```

Both `(100 - discountPercent)` and `100` are `Int`s, so Papyrus performs
`Int / Int` division — truncating toward zero — *before* the result
widens into the `Float`-typed `remainingFraction`. A 25% discount (`75 /
100`) becomes `0`, not `0.75`, so every customer pays nothing. The
**Int/Int division widened to Float** lint (`int-division-to-float`)
flags this; casting an operand (not the result) fixes it:

```papyrus
ScriptName Example

Function ApplyDiscount(Int price, Int discountPercent)
    Float remainingFraction = (100 - discountPercent) / 100 as Float
    Debug.Notification("You pay " + (price * remainingFraction))
EndFunction
```

## Unguarded self-recursion

```papyrus
ScriptName Example

Function CountDown(Int seconds)
    Debug.Notification(seconds as String)
    Utility.Wait(1.0)
    CountDown(seconds - 1)
EndFunction
```

Nothing here ever stops `CountDown` from calling itself again, so it
recurses unconditionally on every invocation until the call stack is
exhausted. The **Unguarded self-recursion** lint
(`unguarded-self-recursion`) flags a self-call with no guard that could
ever skip it; a `Return` guard fixes it:

```papyrus
ScriptName Example

Function CountDown(Int seconds)
    If seconds <= 0
        Return
    EndIf
    Debug.Notification(seconds as String)
    Utility.Wait(1.0)
    CountDown(seconds - 1)
EndFunction
```

## Short update interval

```papyrus
ScriptName Example extends ObjectReference

Event OnInit()
    RegisterForSingleUpdate(0.01)
EndEvent

Event OnUpdate()
    ; ...
    RegisterForSingleUpdate(0.01)
EndEvent
```

A 0.01-second update interval runs far more often than is typically
useful, and adds up to meaningful performance overhead across many
active references. The **Short wait/update interval** lint
(`short-wait-interval`) flags a `RegisterForSingleUpdate`/`RegisterForUpdate`/
`Utility.Wait` interval below the configurable `min_wait_interval`
(default `0.1`).

## Discarded getter result

```papyrus
ScriptName Example extends ObjectReference

Function CheckDistance(Actor akTarget)
    GetDistance(akTarget) > 500.0
EndFunction
```

The comparison's result is never assigned, returned, or used in a
condition — it's computed and immediately thrown away, which is almost
always a forgotten `If` rather than something intentional. The
**Getter usage without saving result** lint (`unused-getter`) flags a
`Get*` call (or an expression built from one) whose result is discarded
this way.

---

See the README's [Implemented Lints table](../README.md#implemented-lints)
for the full list, including the purely style/formatting lints not shown
here, and [Disabling a lint on a specific
line](../README.md#disabling-a-lint-on-a-specific-line) for suppressing a
single false positive with `; @disable`.
