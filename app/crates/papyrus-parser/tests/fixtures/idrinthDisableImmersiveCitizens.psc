Scriptname idrinthDisableImmersiveCitizens extends Quest

Actor[] Property candidates  Auto

Event OnInit()
	Faction exclude = Game.GetFormFromFile(0x237FB4, "ImmersiveCiticens - AI Overhault.esp") As Faction;
	if exclude
		int pos = 0;
		while pos < candidates.Length
			candidates[pos].AddToFaction(exclude);
			pos = pos + 1;
		endWhile
	endIf
EndEvent
