import { describe, expect, it } from "vitest";
import {
  makePickerController,
  makePicker,
  rankItems,
  type PickerItem,
  type PickerSource,
  type PickerSourceResult,
} from "../src/index.js";

const item = (id: string, extra?: Partial<PickerItem<string>>): PickerItem<string> => ({
  id,
  label: id,
  data: id,
  ...extra,
});

const memorySource = (items: ReadonlyArray<PickerItem<string>>): PickerSource<string> => ({
  id: "memory",
  query: () => ({ items }),
});

const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

describe("rankItems", () => {
  it("filters non-matches and ranks exact > prefix > word-prefix > substring, stably", () => {
    const items = [
      item("open recent", { label: "Open Recent" }),
      item("file open", { label: "File: Open", searchTerms: ["file.open", "e", "edit"] }),
      item("open", { label: "Open" }),
      item("reopen", { label: "Reopen Closed Tab" }),
      item("unrelated", { label: "Split Down" }),
    ];
    const ranked = rankItems(items, "open");
    expect(ranked.map((entry) => entry.id)).toEqual([
      "open", // exact
      "open recent", // prefix
      "file open", // word-prefix ("open" after "File: ")
      "reopen", // substring
    ]);
  });

  it("keeps source order (group clusters) at the empty query", () => {
    const items = [item("b"), item("a"), item("c")];
    expect(rankItems(items, "")).toEqual(items);
  });

  it("matches against searchTerms (aliases), not just the label", () => {
    const items = [item("save", { label: "Save File", searchTerms: ["w", "write"] })];
    expect(rankItems(items, "w").length).toBe(1);
    expect(rankItems(items, "nope").length).toBe(0);
  });
});

describe("makePickerController", () => {
  it("opens with an immediate empty-query pass and ranked items", () => {
    const controller = makePickerController();
    controller.open({
      source: memorySource([item("alpha"), item("beta")]),
      onChoose: () => {},
    });
    const state = controller.state();
    expect(state.open).toBe(true);
    expect(state.items.map((entry) => entry.id)).toEqual(["alpha", "beta"]);
    expect(state.selectedId).toBe("alpha");
  });

  it("honors initialSelectedId when present in the results", () => {
    const controller = makePickerController();
    controller.open({
      source: memorySource([item("alpha"), item("beta")]),
      initialSelectedId: "beta",
      onChoose: () => {},
    });
    expect(controller.state().selectedId).toBe("beta");
  });

  it("choose fires onChoose (not onDismiss) and closes", () => {
    const chosen: string[] = [];
    let dismissed = 0;
    const controller = makePickerController();
    controller.open({
      source: memorySource([item("alpha"), item("beta")]),
      onChoose: (entry) => chosen.push(entry.data as string),
      onDismiss: () => dismissed++,
    });
    controller.setSelected("beta");
    controller.choose();
    expect(chosen).toEqual(["beta"]);
    expect(dismissed).toBe(0);
    expect(controller.state().open).toBe(false);
  });

  it("dismiss fires onDismiss (the revert seam) and not onChoose", () => {
    let dismissed = 0;
    const controller = makePickerController();
    controller.open({
      source: memorySource([item("alpha")]),
      onChoose: () => {
        throw new Error("must not choose");
      },
      onDismiss: () => dismissed++,
    });
    controller.dismiss();
    expect(dismissed).toBe(1);
    expect(controller.state().open).toBe(false);
  });

  it("fires onSelectionChange on open, on move, and dedupes repeats", () => {
    const seen: Array<string | null> = [];
    const controller = makePickerController();
    controller.open({
      source: memorySource([item("alpha"), item("beta")]),
      onChoose: () => {},
      onSelectionChange: (entry) => seen.push(entry?.id ?? null),
    });
    controller.setSelected("beta");
    controller.setSelected("beta");
    controller.setSelected("missing");
    expect(seen).toEqual(["alpha", "beta"]);
  });

  it("drops stale async answers: the newest query owns the results", async () => {
    const resolvers = new Map<string, (result: PickerSourceResult<string>) => void>();
    const controller = makePickerController();
    controller.open({
      source: {
        id: "async",
        query: (query) =>
          query.length === 0
            ? { items: [] }
            : new Promise<PickerSourceResult<string>>((resolve) => resolvers.set(query, resolve)),
      },
      onChoose: () => {},
    });
    controller.setQuery("first");
    controller.setQuery("second");
    // The stale answer resolves late — it must be dropped.
    resolvers.get("second")!({ items: [item("second-hit")] });
    await tick();
    resolvers.get("first")!({ items: [item("first-hit")] });
    await tick();
    expect(controller.state().items.map((entry) => entry.id)).toEqual(["second-hit"]);
    expect(controller.state().loading).toBe(false);
  });

  it("debounces backend sources but runs the empty query immediately", async () => {
    let calls = 0;
    const controller = makePickerController();
    controller.open({
      source: {
        id: "debounced",
        debounceMs: 20,
        query: (query) => {
          calls++;
          return { items: [item(`hit:${query}`)] };
        },
      },
      onChoose: () => {},
    });
    expect(calls).toBe(1); // the open pass, no debounce
    controller.setQuery("a");
    controller.setQuery("ab");
    controller.setQuery("abc");
    expect(calls).toBe(1); // all coalescing
    expect(controller.state().loading).toBe(true);
    await wait(40);
    expect(calls).toBe(2); // one trailing query
    expect(controller.state().items[0]?.id).toBe("hit:abc");
    controller.setQuery(""); // clearing bypasses the debounce
    expect(calls).toBe(3);
  });

  it("preRanked sources bypass the built-in ranking", () => {
    const controller = makePickerController();
    controller.open({
      source: {
        id: "ranked",
        preRanked: true,
        query: () => ({ items: [item("zzz no match at all"), item("also unrelated")] }),
      },
      onChoose: () => {},
    });
    controller.setQuery("query-that-matches-nothing");
    // preRanked: the controller must not filter these away.
    expect(controller.state().items.length).toBe(2);
  });

  it("surfaces source failures as error state, not silence", async () => {
    const controller = makePickerController();
    controller.open({
      source: {
        id: "failing",
        query: (query) =>
          query.length === 0 ? { items: [] } : Promise.reject(new Error("index unavailable")),
      },
      onChoose: () => {},
    });
    controller.setQuery("x");
    await tick();
    expect(controller.state().error).toBe("index unavailable");
    expect(controller.state().items).toEqual([]);
  });

  it("opening over an open picker dismisses the first (revert runs)", () => {
    let firstDismissed = 0;
    const controller = makePickerController();
    controller.open({
      source: memorySource([item("one")]),
      onChoose: () => {},
      onDismiss: () => firstDismissed++,
    });
    controller.open({
      source: memorySource([item("two")]),
      onChoose: () => {},
    });
    expect(firstDismissed).toBe(1);
    expect(controller.state().items[0]?.id).toBe("two");
  });

  it("makePicker exposes open/close over the controller", () => {
    const controller = makePickerController();
    const picker = makePicker(controller);
    picker.open({ source: memorySource([item("alpha")]), onChoose: () => {} });
    expect(controller.state().open).toBe(true);
    picker.close();
    expect(controller.state().open).toBe(false);
  });
});
