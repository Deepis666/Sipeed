import { describe, it, expect } from "vitest"
import { cn, valueUpdater } from "../utils"
import { ref, type Ref } from "vue"

describe("cn()", () => {
  it("returns a single class string as-is", () => {
    expect(cn("foo")).toBe("foo")
  })

  it("merges multiple class strings", () => {
    expect(cn("foo", "bar")).toBe("foo bar")
  })

  it("resolves conflicting Tailwind classes (last wins)", () => {
    expect(cn("px-4", "px-2")).toBe("px-2")
  })

  it("filters out falsy values", () => {
    expect(cn("foo", false, null, undefined, "", "bar")).toBe("foo bar")
  })

  it("handles conditional class objects", () => {
    expect(cn({ active: true, inactive: false })).toBe("active")
  })

  it("handles nested arrays of classes", () => {
    expect(cn("a", ["b", ["c", "d"]])).toBe("a b c d")
  })

  it("returns empty string for empty input", () => {
    expect(cn()).toBe("")
  })
})

describe("valueUpdater()", () => {
  it("sets the ref value to a direct value", () => {
    const count: Ref<number> = ref(0)
    valueUpdater(42, count)
    expect(count.value).toBe(42)
  })

  it("sets the ref value using a function updater", () => {
    const count: Ref<number> = ref(10)
    valueUpdater((prev: number) => prev + 5, count)
    expect(count.value).toBe(15)
  })

  it("works with string values", () => {
    const name: Ref<string> = ref("hello")
    valueUpdater("world", name)
    expect(name.value).toBe("world")

    valueUpdater((prev: unknown) => (prev as string).toUpperCase(), name)
    expect(name.value).toBe("WORLD")
  })

  it("works with object values", () => {
    const state = ref({ x: 1, y: 2 })
    valueUpdater({ x: 10, y: 20 }, state)
    expect(state.value).toEqual({ x: 10, y: 20 })
  })
})
