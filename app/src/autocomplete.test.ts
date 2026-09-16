import { describe, expect, it } from "vitest";
import {
  type Member,
  completionInsertText,
  completionLabel,
  completionQueryAt,
  declarationDocumentationOnLine,
  declaredTypes,
  documentationByName,
  documentationForIdentifier,
  filterMembers,
  identifierAt,
  memberDocumentation,
  overlayLocalDocumentation,
  stripComments,
} from "./autocomplete";

const SCRIPT = `ScriptName Example Extends Quest

ObjectReference Property TargetRef Auto

Function DoThing(Actor akSpeaker, ObjectReference[] akRefs)
    Int localCount = 0
    self.DoThing(akSpeaker, akRefs)
    parent.DoThing(akSpeaker, akRefs)
EndFunction
`;

describe("stripComments", () => {
  it("blanks out block, brace, and line comments while preserving layout", () => {
    const source = "Int i ; a line comment\n{a brace comment}\n;/ a block\ncomment /;\nFloat f\n";
    const stripped = stripComments(source);
    expect(stripped).not.toContain("a line comment");
    expect(stripped).not.toContain("a brace comment");
    expect(stripped).not.toContain("a block");
    // Line count (and therefore later regex matching) is unaffected.
    expect(stripped.split("\n").length).toBe(source.split("\n").length);
  });

  it("preserves every character position outside line endings", () => {
    const source = "Int before ; comment\n{brace}\n;/ block /; Float after";
    const stripped = stripComments(source);

    expect(stripped).toHaveLength(source.length);
    expect(stripped.indexOf("Float after")).toBe(source.indexOf("Float after"));
    expect([...stripped].filter((character) => character === "\n")).toHaveLength(2);
  });

  it("blanks unterminated block and brace comments through end of source", () => {
    expect(stripComments("Int i\n;/ unfinished\nFloat f")).toBe("Int i\n             \n       ");
    expect(stripComments("Int i\n{unfinished\nFloat f")).toBe("Int i\n           \n       ");
  });
});

describe("declaredTypes", () => {
  it("maps self/parent to the script's own name and its Extends parent", () => {
    const types = declaredTypes(SCRIPT);
    expect(types.get("self")).toBe("Example");
    expect(types.get("parent")).toBe("Quest");
  });

  it("maps a Property declaration's name to its type", () => {
    expect(declaredTypes(SCRIPT).get("targetref")).toBe("ObjectReference");
  });

  it("maps function parameters (including array-typed ones) to their type", () => {
    const types = declaredTypes(SCRIPT);
    expect(types.get("akspeaker")).toBe("Actor");
    expect(types.get("akrefs")).toBe("ObjectReference");
  });

  it("maps a plain local variable declaration to its type", () => {
    expect(declaredTypes(SCRIPT).get("localcount")).toBe("Int");
  });

  it("leaves self/parent unset when there's no ScriptName line to resolve them from", () => {
    const types = declaredTypes("Int i = 0\n");
    expect(types.has("self")).toBe(false);
    expect(types.has("parent")).toBe(false);
  });

  it("ignores commented-out declarations", () => {
    const source = "ScriptName Example\n; ObjectReference akRef\n";
    expect(declaredTypes(source).has("akref")).toBe(false);
  });

  it("does not mistake a keyword-led statement for a declaration", () => {
    const source = "ScriptName Example\n\nFunction Run()\n    If bReady\n    EndIf\nEndFunction\n";
    expect(declaredTypes(source).has("bready")).toBe(false);
  });

  it("does not mistake an assignment or a call for a declaration", () => {
    const source = "ScriptName Example\n\nFunction Run()\n    akRef = None\n    akRef.MoveTo(akTarget)\nEndFunction\n";
    expect(declaredTypes(source).has("akref")).toBe(false);
  });

  it("maps event parameters and script-level fields", () => {
    const source = `ScriptName Example
Actor CurrentActor

Event OnHit(ObjectReference akAggressor, Form[] akSources)
EndEvent
`;
    const types = declaredTypes(source);

    expect(types.get("currentactor")).toBe("Actor");
    expect(types.get("akaggressor")).toBe("ObjectReference");
    expect(types.get("aksources")).toBe("Form");
  });

  it("rejects declarations that use a keyword as the type or variable name", () => {
    const types = declaredTypes("If possible = None\nActor Return = None\n");

    expect(types.has("possible")).toBe(false);
    expect(types.has("return")).toBe(false);
  });
});

describe("completionQueryAt", () => {
  it("resolves a query right after typing a receiver's dot", () => {
    const cursor = SCRIPT.indexOf("self.DoThing") + "self.".length;
    const query = completionQueryAt(SCRIPT, cursor);
    expect(query).toEqual({ receiverType: "Example", prefix: "", prefixStart: cursor });
  });

  it("resolves a query mid-way through typing a member name", () => {
    const partial = SCRIPT.replace("self.DoThing", "self.DoTh");
    const cursor = partial.indexOf("self.DoTh") + "self.DoTh".length;
    const query = completionQueryAt(partial, cursor);
    expect(query).toEqual({ receiverType: "Example", prefix: "DoTh", prefixStart: cursor - 4 });
  });

  it("resolves parent. to the script's Extends parent", () => {
    const cursor = SCRIPT.indexOf("parent.DoThing") + "parent.".length;
    expect(completionQueryAt(SCRIPT, cursor)?.receiverType).toBe("Quest");
  });

  it("returns null when the cursor isn't right after a member access", () => {
    expect(completionQueryAt("Int i = 0", 5)).toBeNull();
  });

  it("returns null when the receiver's type isn't known", () => {
    const source = "ScriptName Example\n\nFunction Run()\n    unknownVar.\nEndFunction\n";
    expect(completionQueryAt(source, source.indexOf("unknownVar.") + "unknownVar.".length)).toBeNull();
  });

  it("resolves an indexed array element's declared element type", () => {
    const source = "ScriptName Example\n\nActor[] actors\n\nFunction Run()\n    actors[0].\nEndFunction\n";
    const cursor = source.indexOf("actors[0].") + "actors[0].".length;
    expect(completionQueryAt(source, cursor)).toEqual({ receiverType: "Actor", prefix: "", prefixStart: cursor });
  });

  it("resolves an indexed array element mid-way through typing a member name", () => {
    const source = "ScriptName Example\n\nActor[] actors\n\nFunction Run()\n    actors[0].Disa\nEndFunction\n";
    const cursor = source.indexOf("actors[0].Disa") + "actors[0].Disa".length;
    expect(completionQueryAt(source, cursor)).toEqual({ receiverType: "Actor", prefix: "Disa", prefixStart: cursor - 4 });
  });

  it("resolves an indexed array element whose index is a variable", () => {
    const source = "ScriptName Example\n\nActor[] actors\n\nFunction Run()\n    actors[i].\nEndFunction\n";
    const cursor = source.indexOf("actors[i].") + "actors[i].".length;
    expect(completionQueryAt(source, cursor)?.receiverType).toBe("Actor");
  });

  it("allows whitespace inside an array index and before the member-access dot", () => {
    const source = "ScriptName Example\nActor[] actors\nactors[ index + 1 ] .Get";
    const cursor = source.length;

    expect(completionQueryAt(source, cursor)).toEqual({
      receiverType: "Actor",
      prefix: "Get",
      prefixStart: cursor - 3,
    });
  });

  it("uses only the source before the cursor to identify the typed prefix", () => {
    const source = "ScriptName Example\nActor target\ntarget.GetName()";
    const cursor = source.indexOf("GetName") + 3;

    expect(completionQueryAt(source, cursor)).toEqual({
      receiverType: "Actor",
      prefix: "Get",
      prefixStart: cursor - 3,
    });
  });

  it("does not resolve a function-call result as a simple receiver", () => {
    const source = "ScriptName Example\nActor target\ntarget.GetActor().GetName";

    expect(completionQueryAt(source, source.length)).toBeNull();
  });
});

describe("filterMembers", () => {
  const members: Member[] = [
    { kind: "property", name: "TargetRef", type_name: { name: "ObjectReference", is_array: false } },
    {
      kind: "function",
      name: "GetName",
      params: [],
      return_type: { name: "String", is_array: false },
      is_global: false,
      is_native: true,
      is_event: false,
    },
    {
      kind: "function",
      name: "getActorValue",
      params: [{ name: "avName", type_name: { name: "String", is_array: false } }],
      return_type: { name: "Float", is_array: false },
      is_global: false,
      is_native: true,
      is_event: false,
    },
  ];

  it("filters case-insensitively by name prefix", () => {
    expect(filterMembers(members, "get").map((m) => m.name).sort()).toEqual(["GetName", "getActorValue"].sort());
  });

  it("sorts matches alphabetically", () => {
    const names = filterMembers(members, "").map((m) => m.name);
    expect(new Set(names)).toEqual(new Set(["GetName", "TargetRef", "getActorValue"]));
    expect(names.indexOf("TargetRef")).toBeGreaterThan(names.indexOf("GetName"));
    expect(names.indexOf("TargetRef")).toBeGreaterThan(names.indexOf("getActorValue"));
  });

  it("returns nothing when no member matches", () => {
    expect(filterMembers(members, "zzz")).toEqual([]);
  });

  it("does not reorder the backend's original member array", () => {
    const originalOrder = members.map((member) => member.name);

    filterMembers(members, "");

    expect(members.map((member) => member.name)).toEqual(originalOrder);
  });
});

describe("completionLabel", () => {
  it("labels a property with its type", () => {
    const member: Member = { kind: "property", name: "TargetRef", type_name: { name: "ObjectReference", is_array: false } };
    expect(completionLabel(member)).toBe("TargetRef: ObjectReference");
  });

  it("labels a function with its parameters (name and type) and return type", () => {
    const member: Member = {
      kind: "function",
      name: "GetActorValue",
      params: [{ name: "avName", type_name: { name: "String", is_array: false } }],
      return_type: { name: "Float", is_array: false },
      is_global: false,
      is_native: true,
      is_event: false,
    };
    expect(completionLabel(member)).toBe("GetActorValue(String avName) -> Float");
  });

  it("marks an array-typed property/parameter with []", () => {
    const property: Member = { kind: "property", name: "Targets", type_name: { name: "ObjectReference", is_array: true } };
    expect(completionLabel(property)).toBe("Targets: ObjectReference[]");

    const fn: Member = {
      kind: "function",
      name: "Sum",
      params: [{ name: "values", type_name: { name: "Int", is_array: true } }],
      return_type: { name: "Int", is_array: true },
      is_global: true,
      is_native: false,
      is_event: false,
    };
    expect(completionLabel(fn)).toBe("Sum(Int[] values) -> Int[]");
  });

  it("labels a function with no return type without an arrow", () => {
    const member: Member = {
      kind: "function",
      name: "Wait",
      params: [{ name: "duration", type_name: { name: "Float", is_array: false } }],
      return_type: null,
      is_global: true,
      is_native: true,
      is_event: false,
    };
    expect(completionLabel(member)).toBe("Wait(Float duration)");
  });
});

describe("completionInsertText", () => {
  it("inserts just the name for a property", () => {
    const member: Member = { kind: "property", name: "TargetRef", type_name: { name: "ObjectReference", is_array: false } };
    expect(completionInsertText(member)).toBe("TargetRef");
  });

  it("inserts the name plus an opening paren for a function", () => {
    const member: Member = {
      kind: "function",
      name: "GetName",
      params: [],
      return_type: { name: "String", is_array: false },
      is_global: false,
      is_native: true,
      is_event: false,
    };
    expect(completionInsertText(member)).toBe("GetName(");
  });
});

describe("documentationByName", () => {
  it("maps ScriptName, Property, Function, and Event declarations to their brace comments", () => {
    const source = `ScriptName Example Extends Quest
{A documented script}

Int Property TargetRef Auto
{Where we go}

Function DoThing()
{Does the thing}
EndFunction

Event OnInit()
{Runs on init}
EndEvent
`;
    const docs = documentationByName(source);
    expect(docs.get("example")).toBe("A documented script");
    expect(docs.get("self")).toBe("A documented script");
    expect(docs.get("targetref")).toBe("Where we go");
    expect(docs.get("dothing")).toBe("Does the thing");
    expect(docs.get("oninit")).toBe("Runs on init");
  });

  it("keeps inner newlines of a multi-line brace comment", () => {
    const source = "ScriptName Example\n{First line\nSecond line}\n";
    expect(documentationByName(source).get("example")).toBe("First line\nSecond line");
  });

  it("follows a backslash-continued function header", () => {
    const source = "ScriptName Example\n\nFunction DoThing(Int a, \\\n    Int b)\n{Explains what DoThing does}\nEndFunction\n";
    expect(documentationByName(source).get("dothing")).toBe("Explains what DoThing does");
  });

  it("ignores commented-out declarations and empty brace comments", () => {
    const source = "ScriptName Example\n{   }\n; Function Hidden()\n; {secret}\n";
    const docs = documentationByName(source);
    expect(docs.has("example")).toBe(false);
    expect(docs.has("hidden")).toBe(false);
  });
});

describe("declarationDocumentationOnLine", () => {
  it("returns the doc of the declaration whose header is on that line", () => {
    const source = "ScriptName Example\n{script doc}\n\nFunction DoThing()\n{fn doc}\nEndFunction\n";
    expect(declarationDocumentationOnLine(source, 1)).toBe("script doc");
    expect(declarationDocumentationOnLine(source, 4)).toBe("fn doc");
    expect(declarationDocumentationOnLine(source, 2)).toBeNull();
    expect(declarationDocumentationOnLine(source, 5)).toBeNull();
  });
});

describe("identifierAt", () => {
  it("returns the identifier under the index, including at its end", () => {
    const source = "Function DoThing()";
    const start = source.indexOf("DoThing");
    expect(identifierAt(source, start)).toMatchObject({ name: "DoThing", receiver: null });
    expect(identifierAt(source, start + "DoThing".length)).toMatchObject({ name: "DoThing", receiver: null });
  });

  it("captures the receiver of a member access", () => {
    const source = "self.DoThing(";
    const ident = identifierAt(source, source.indexOf("DoThing"));
    expect(ident).toMatchObject({ name: "DoThing", receiver: "self" });
  });

  it("does not resolve an identifier inside a brace comment", () => {
    const source = "Function DoThing()\n{mentions DoThing}\n";
    expect(identifierAt(source, source.lastIndexOf("DoThing"))).toBeNull();
  });
});

describe("documentationForIdentifier / overlayLocalDocumentation", () => {
  const source = `ScriptName Example
{A documented script}

Function DoThing()
{Does the thing}
EndFunction
`;

  it("resolves a bare declaration name from the current buffer", () => {
    const ident = identifierAt(source, source.indexOf("DoThing"));
    expect(ident).not.toBeNull();
    expect(documentationForIdentifier(source, ident!, () => undefined)).toBe("Does the thing");
  });

  it("resolves the script's own name, and self, to the script header's documentation", () => {
    const ident = identifierAt(source, source.indexOf("Example"));
    expect(documentationForIdentifier(source, ident!, () => undefined)).toBe("A documented script");

    const withSelf = `${source}\nFunction Run()\n    self.DoThing()\nEndFunction\n`;
    const selfIdent = identifierAt(withSelf, withSelf.indexOf("self"));
    expect(documentationForIdentifier(withSelf, selfIdent!, () => undefined)).toBe("A documented script");
  });

  it("prefers an unsaved local doc over the backend member list for self", () => {
    const callSite = `${source}\nFunction Run()\n    self.DoThing()\nEndFunction\n`;
    const ident = identifierAt(callSite, callSite.lastIndexOf("DoThing"));
    expect(ident?.receiver).toBe("self");
    expect(
      documentationForIdentifier(callSite, ident!, () => [
        {
          kind: "function",
          name: "DoThing",
          params: [],
          return_type: null,
          is_global: false,
          is_native: false,
          is_event: false,
          doc: "stale disk copy",
        },
      ]),
    ).toBe("Does the thing");
  });

  it("falls back to the backend member list for a foreign receiver", () => {
    const callSite = "ScriptName Example\nObjectReference akRef\nakRef.MoveTo\n";
    const ident = identifierAt(callSite, callSite.indexOf("MoveTo"));
    expect(ident?.receiver).toBe("akRef");
    const members: Member[] = [
      {
        kind: "function",
        name: "MoveTo",
        params: [],
        return_type: null,
        is_global: false,
        is_native: true,
        is_event: false,
        doc: "Moves this reference",
      },
    ];
    expect(documentationForIdentifier(callSite, ident!, (type) => (type === "ObjectReference" ? members : undefined))).toBe(
      "Moves this reference",
    );
  });

  it("overlays local docs onto members of the script currently being edited", () => {
    const members: Member[] = [
      {
        kind: "function",
        name: "DoThing",
        params: [],
        return_type: null,
        is_global: false,
        is_native: false,
        is_event: false,
        doc: "stale disk copy",
      },
    ];
    const overlaid = overlayLocalDocumentation(members, source, "Example");
    expect(memberDocumentation(overlaid[0])).toBe("Does the thing");
    expect(memberDocumentation(members[0])).toBe("stale disk copy");
  });

  it("does not overlay local docs onto a different receiver type", () => {
    const members: Member[] = [
      {
        kind: "function",
        name: "DoThing",
        params: [],
        return_type: null,
        is_global: false,
        is_native: false,
        is_event: false,
        doc: "from Quest",
      },
    ];
    const overlaid = overlayLocalDocumentation(members, source, "Quest");
    expect(memberDocumentation(overlaid[0])).toBe("from Quest");
  });
});
