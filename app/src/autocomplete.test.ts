import { describe, expect, it } from "vitest";
import {
  type Member,
  completionInsertText,
  completionLabel,
  completionQueryAt,
  declaredTypes,
  filterMembers,
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
