Scriptname VahlokInheritProbe extends Quest

; Papyrus hierarchy: Actor extends ObjectReference extends Form.
ObjectReference Property AnObjRef Auto
Actor           Property AnActor  Auto
Form            Property AForm    Auto
Spell           Property ASpell   Auto

Function Takes(ObjectReference akRef)
	Debug.Trace("" + akRef)
EndFunction

Function Probe()
	Takes(AnObjRef)   ; exact type        -> silent
	Takes(AnActor)    ; UPCAST, legal     -> should be silent
	Takes(AForm)      ; DOWNCAST, illegal -> should report
	Takes(ASpell)     ; SIBLING, illegal  -> should report
EndFunction

ObjectReference Function ReturnsUpcast()
	return AnActor    ; UPCAST, legal     -> should be silent
EndFunction

ObjectReference Function ReturnsSibling()
	return ASpell     ; SIBLING, illegal  -> should report
EndFunction
