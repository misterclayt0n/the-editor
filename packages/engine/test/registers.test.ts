import { describe, expect, test } from "vitest";
import {
  InvalidRegisterNameError,
  RegisterData,
  Registers,
  isValidRegisterName,
} from "../src/index";

describe("registers", () => {
  test("new registers are empty", () => {
    const registers = Registers.create();
    expect(registers.isEmpty).toBe(true);
    expect(registers.size).toBe(0);
    expect(registers.read("a")).toBeUndefined();
  });

  test("write/read preserve fragment order", () => {
    const registers = Registers.create();
    registers.write("a", RegisterData.text(["first", "second"]));
    expect(registers.read("a")).toEqual(RegisterData.text(["first", "second"]));
    expect(registers.first("a")).toBe("first");
    expect(registers.last("a")).toBe("second");
  });

  test("push appends", () => {
    const registers = Registers.create();
    registers.write("a", RegisterData.text(["first"]));
    registers.push("a", "second");
    registers.push("a", "third");
    expect(registers.read("a")).toEqual(RegisterData.text(["first", "second", "third"]));
  });

  test("push preserves existing kind", () => {
    const registers = Registers.create();
    registers.write("l", RegisterData.line(["first"]));
    registers.push("l", "second");
    expect(registers.read("l")).toEqual(RegisterData.line(["first", "second"]));

    registers.write("b", RegisterData.block(["row1"]));
    registers.push("b", "row2");
    expect(registers.read("b")).toEqual(RegisterData.block(["row1", "row2"]));
  });

  test("write returns previous value", () => {
    const registers = Registers.create();
    expect(registers.write("a", RegisterData.text("first"))).toBeUndefined();
    expect(registers.write("a", RegisterData.text("second"))).toEqual(RegisterData.text("first"));
  });

  test("remove and clear", () => {
    const registers = Registers.create();
    registers.write("a", RegisterData.text("one"));
    registers.write("b", RegisterData.text("two"));
    expect(registers.remove("a")).toEqual(RegisterData.text("one"));
    expect(registers.has("a")).toBe(false);
    expect(registers.has("b")).toBe(true);
    registers.clear();
    expect(registers.isEmpty).toBe(true);
  });

  test("entries iterate in key order", () => {
    const registers = Registers.create();
    registers.write("z", RegisterData.text("last"));
    registers.write("a", RegisterData.text("first"));
    registers.write("m", RegisterData.text("middle"));
    expect([...registers.entries()].map(([name]) => name)).toEqual(["a", "m", "z"]);
  });

  test("register name validation", () => {
    expect(isValidRegisterName("a")).toBe(true);
    expect(isValidRegisterName("+")).toBe(true);
    expect(isValidRegisterName("😀")).toBe(true);
    expect(isValidRegisterName("")).toBe(false);
    expect(isValidRegisterName("ab")).toBe(false);
    const registers = Registers.create();
    expect(() => registers.read("")).toThrow(InvalidRegisterNameError);
    expect(() => registers.write("ab", RegisterData.text("x"))).toThrow(InvalidRegisterNameError);
  });

  test("defensive cloning", () => {
    const registers = Registers.create();
    const fragments = ["a"];
    registers.write("x", RegisterData.text(fragments));
    fragments.push("mutated");
    expect(registers.read("x")).toEqual(RegisterData.text(["a"]));

    const read = registers.read("x")!;
    expect(Object.isFrozen(read.type === "block" ? read.rows : read.fragments)).toBe(true);
    expect(() =>
      ((read.type === "block" ? read.rows : read.fragments) as string[]).push("mutated"),
    ).toThrow();
    expect(registers.read("x")).toEqual(RegisterData.text(["a"]));
  });
});
