import { describe, expect, it } from "vitest";
import { nodiscardEligibleLines } from "./nodiscard";

describe("nodiscardEligibleLines", () => {
  it("flags a function that returns a value", () => {
    expect(nodiscardEligibleLines("Int Function GetValue()\n    Return 1\nEndFunction\n")).toEqual(new Set([1]));
  });

  it("flags a native function with no return value", () => {
    expect(nodiscardEligibleLines("Function DoThing() Native\n")).toEqual(new Set([1]));
  });

  it("flags a native function that also returns a value", () => {
    expect(nodiscardEligibleLines("Int Function GetValue() Native\n")).toEqual(new Set([1]));
  });

  it("does not flag a void, non-native function", () => {
    expect(nodiscardEligibleLines("Function DoThing()\nEndFunction\n")).toEqual(new Set());
  });

  it("does not flag an Event declaration", () => {
    expect(nodiscardEligibleLines("Event OnInit()\nEndEvent\n")).toEqual(new Set());
  });

  it("does not flag a header already marked @nodiscard on its own line", () => {
    expect(nodiscardEligibleLines("Int Function GetValue() ; @nodiscard\n")).toEqual(new Set());
  });

  it("does not flag a header already marked @nodiscard on the line above", () => {
    expect(nodiscardEligibleLines("; @nodiscard\nInt Function GetValue()\n")).toEqual(new Set());
  });

  it("still flags a header whose above-line comment is @nodiscardable, not @nodiscard", () => {
    expect(nodiscardEligibleLines("; @nodiscardable\nInt Function GetValue()\n")).toEqual(new Set([2]));
  });

  it("ignores the word Native or a return-type-shaped word inside a comment", () => {
    expect(nodiscardEligibleLines("Function DoThing() ; behaves like a Native call\nEndFunction\n")).toEqual(
      new Set(),
    );
  });

  it("reports every eligible line across a multi-function script", () => {
    const source = "Int Function GetValue()\n    Return 1\nEndFunction\n\nFunction DoThing()\nEndFunction\n";
    expect(nodiscardEligibleLines(source)).toEqual(new Set([1]));
  });
});
