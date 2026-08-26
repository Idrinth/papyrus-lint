;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
;NEXT FRAGMENT INDEX 1
Scriptname IDR__TIF__050002AB Extends TopicInfo Hidden

;BEGIN FRAGMENT Fragment_0
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
int pos = 0;
while pos < idrinthAlyienethAllowedFoodRecipesResults.GetSize()
	Potion food = idrinthAlyienethAllowedFoodRecipesResults.GetAt(pos) As potion;
	int cnt = idrinthAlyienethContainerFoodCookedRef.GetItemCount(food);
	if cnt > 0
		idrinthAlyienethContainerFoodCookedRef.RemoveItem(food, cnt, false, PlayerRef);
	endIf
	pos = pos + 1;
EndWhile
;END CODE
EndFunction
;END FRAGMENT

;END FRAGMENT CODE - Do not edit anything between this and the begin comment

FormList Property idrinthAlyienethAllowedFoodRecipesResults  Auto  

ObjectReference Property idrinthAlyienethContainerFoodCookedRef  Auto  

Actor Property PlayerRef  Auto  
