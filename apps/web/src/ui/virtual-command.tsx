/**
 * A virtualized, data-driven command list — our replacement for cmdk's render-
 * everything model. cmdk mounts one subscribing DOM node per item and, on each
 * keystroke, scores all N, reorders N DOM nodes, and re-renders 2N
 * subscriptions; past a couple thousand items that is unusable (the `/tree`
 * picker over a long session). This renders only the visible window
 * (`viewport / rowHeight + overscan` rows, ~30 nodes) regardless of list size,
 * filters in plain JS, and keeps selection in React state — no per-item
 * subscriptions. The shell (`CommandDialog`) and `.command-palette` CSS are
 * unchanged; callers pass rows as data and a `renderRow`, instead of mapping to
 * self-registering `<CommandItem>` children.
 *
 * Scope note: plain React state + windowing is enough at command-palette scale.
 * Pierre's file tree uses an external store + slot-keyed node reuse because it
 * windows 100k-row trees under continuous scroll; a palette doesn't, so we keep
 * the simpler model.
 */

import {
  type ReactNode,
  type Ref,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

/** The minimum a row must carry. Callers extend this with their own fields and
 *  read them back in `renderRow`. */
export interface VirtualCommandRow {
  /** Stable identity: React key, selection value, and the unit of navigation. */
  readonly id: string;
  /** Non-selectable rows (group headings, separators) are skipped by the
   *  keyboard cursor. Defaults to selectable. */
  readonly selectable?: boolean;
  /** Lowercased haystack for the built-in `filter`. Omit to opt a row out of
   *  matching (it then shows only when the query is empty). */
  readonly searchText?: string;
}

export interface VirtualCommandProps<T extends VirtualCommandRow> {
  readonly rows: ReadonlyArray<T>;
  /** Fixed row height in px — all geometry is `index * rowHeight`. */
  readonly rowHeight: number;
  /** Rows rendered beyond the viewport on each side (default 8). */
  readonly overscan?: number;

  /** Controlled search text. */
  readonly query: string;
  readonly onQueryChange: (query: string) => void;
  readonly placeholder?: string;
  readonly inputRef?: Ref<HTMLInputElement>;
  readonly autoFocus?: boolean;
  /** How the query narrows rows. `false` (default): caller pre-filters, we show
   *  `rows` as-is. `true`: built-in case-insensitive substring on `searchText`.
   *  A predicate: custom matching. Non-selectable rows always pass (the caller
   *  prunes empty groups). */
  readonly filter?: boolean | ((row: T, query: string) => boolean);

  /** Controlled selected id (the highlighted row). Uncontrolled if omitted. */
  readonly value?: string | undefined;
  readonly onValueChange?: (id: string) => void;
  /** Enter or click on a selectable row. */
  readonly onChoose: (row: T) => void;
  /** Wrap the cursor past the ends (default true). */
  readonly loop?: boolean;

  readonly renderRow: (row: T, ctx: { readonly selected: boolean; readonly query: string }) => ReactNode;
  readonly empty?: ReactNode;
  /** A left adornment for the input (e.g. a search icon). */
  readonly icon?: ReactNode;
  /** Chrome above the list, inside the list column (a status/help line). */
  readonly belowInput?: ReactNode;
  /** Chrome under the list, inside the list column (a count footer). */
  readonly footer?: ReactNode;
  /** A panel beside the list (e.g. a file preview). When set, the list and this
   *  panel sit in a flex row under the input. */
  readonly aside?: ReactNode;
  /** Extra key handling on the input, run before the built-in nav. Call
   *  `preventDefault()` to suppress the built-in handling for that key. */
  readonly onInputKeyDown?: (event: React.KeyboardEvent) => void;

  readonly className?: string;
  readonly bodyClassName?: string;
  readonly listColClassName?: string;
  readonly listClassName?: string;
  readonly inputClassName?: string;
}

const norm = (value: string): string => value.trim().toLowerCase();

export function VirtualCommand<T extends VirtualCommandRow>({
  rows,
  rowHeight,
  overscan = 8,
  query,
  onQueryChange,
  placeholder,
  inputRef,
  autoFocus = true,
  filter = false,
  value,
  onValueChange,
  onChoose,
  loop = true,
  renderRow,
  empty,
  icon,
  belowInput,
  footer,
  aside,
  onInputKeyDown,
  className,
  bodyClassName,
  listColClassName,
  listClassName,
  inputClassName,
}: VirtualCommandProps<T>) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const localInputRef = useRef<HTMLInputElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  // Uncontrolled selection fallback.
  const [internalValue, setInternalValue] = useState<string | undefined>(undefined);

  // Narrow the rows. Non-selectable rows (headers) always survive so callers can
  // prune empty groups themselves.
  const visible = useMemo(() => {
    if (filter === false) return rows;
    const q = norm(query);
    if (q.length === 0) return rows;
    const match =
      typeof filter === "function"
        ? (row: T) => filter(row, q)
        : (row: T) => row.searchText !== undefined && row.searchText.includes(q);
    return rows.filter((row) => row.selectable === false || match(row));
  }, [rows, query, filter]);

  const selectableIndexes = useMemo(
    () => visible.reduce<number[]>((acc, row, i) => (row.selectable === false ? acc : (acc.push(i), acc)), []),
    [visible],
  );

  const selectedId = value !== undefined ? value : internalValue;
  const selectedIndex = useMemo(
    () => visible.findIndex((row) => row.id === selectedId && row.selectable !== false),
    [visible, selectedId],
  );

  const setSelected = useCallback(
    (id: string | undefined) => {
      if (value === undefined) setInternalValue(id);
      if (id !== undefined) onValueChange?.(id);
    },
    [value, onValueChange],
  );

  // Keep a valid selection as the list narrows: default to the first selectable
  // row when the current one drops out.
  useEffect(() => {
    if (selectedIndex !== -1) return;
    const first = selectableIndexes[0];
    setSelected(first === undefined ? undefined : visible[first]!.id);
  }, [selectedIndex, selectableIndexes, visible, setSelected]);

  // Track the viewport height without forced reflow.
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (el === null) return;
    setViewportHeight(el.clientHeight);
    const ro = new ResizeObserver((entries) => {
      const box = entries[0]?.contentBoxSize?.[0]?.blockSize ?? entries[0]?.contentRect.height;
      if (typeof box === "number") setViewportHeight(box);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    if (autoFocus) {
      const id = window.requestAnimationFrame(() => (inputRef as { current?: HTMLInputElement } | undefined)?.current?.focus?.() ?? localInputRef.current?.focus());
      return () => window.cancelAnimationFrame(id);
    }
    return undefined;
  }, [autoFocus, inputRef]);

  const total = visible.length;
  const totalHeight = total * rowHeight;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const end = Math.min(total, Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan);
  const windowRows = visible.slice(start, end);
  const offsetTop = start * rowHeight;

  // Manual scroll-into-view (no scrollIntoView): only writes when off-screen.
  const scrollIndexIntoView = useCallback(
    (index: number) => {
      const el = scrollRef.current;
      if (el === null || index < 0) return;
      const top = index * rowHeight;
      const bottom = top + rowHeight;
      if (top < el.scrollTop) el.scrollTop = top;
      else if (bottom > el.scrollTop + el.clientHeight) el.scrollTop = bottom - el.clientHeight;
    },
    [rowHeight],
  );

  const moveCursor = useCallback(
    (delta: 1 | -1) => {
      if (selectableIndexes.length === 0) return;
      const cursor = selectableIndexes.indexOf(selectedIndex);
      let next = cursor + delta;
      if (next < 0) next = loop ? selectableIndexes.length - 1 : 0;
      else if (next >= selectableIndexes.length) next = loop ? 0 : selectableIndexes.length - 1;
      const rowIndex = selectableIndexes[next]!;
      setSelected(visible[rowIndex]!.id);
      scrollIndexIntoView(rowIndex);
    },
    [selectableIndexes, selectedIndex, loop, visible, setSelected, scrollIndexIntoView],
  );

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      onInputKeyDown?.(event);
      if (event.defaultPrevented) return;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        moveCursor(1);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        moveCursor(-1);
      } else if (event.key === "Enter" && !event.nativeEvent.isComposing) {
        const row = selectedIndex === -1 ? undefined : visible[selectedIndex];
        if (row !== undefined) {
          event.preventDefault();
          onChoose(row);
        }
      } else if (event.key === "Home") {
        event.preventDefault();
        const first = selectableIndexes[0];
        if (first !== undefined) {
          setSelected(visible[first]!.id);
          scrollIndexIntoView(first);
        }
      } else if (event.key === "End") {
        event.preventDefault();
        const last = selectableIndexes[selectableIndexes.length - 1];
        if (last !== undefined) {
          setSelected(visible[last]!.id);
          scrollIndexIntoView(last);
        }
      }
    },
    [onInputKeyDown, moveCursor, selectedIndex, visible, onChoose, selectableIndexes, setSelected, scrollIndexIntoView],
  );

  const listColumn = (
    <>
      {belowInput}
      <div
        ref={scrollRef}
        className={listClassName}
        data-slot="command-list"
        role="listbox"
        onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      >
        {total === 0
          ? empty
          : (
            <div style={{ height: totalHeight, position: "relative" }}>
              <div style={{ position: "absolute", top: 0, left: 0, right: 0, transform: `translateY(${offsetTop}px)`, willChange: "transform" }}>
                {windowRows.map((row, i) => {
                  const index = start + i;
                  const selected = index === selectedIndex;
                  const selectable = row.selectable !== false;
                  return (
                    <div
                      key={row.id}
                      role={selectable ? "option" : "presentation"}
                      aria-selected={selectable ? selected : undefined}
                      data-selected={selectable && selected ? "true" : undefined}
                      style={{ height: rowHeight }}
                      onPointerMove={selectable && row.id !== selectedId ? () => setSelected(row.id) : undefined}
                      onClick={selectable ? () => onChoose(row) : undefined}
                    >
                      {renderRow(row, { selected, query })}
                    </div>
                  );
                })}
              </div>
            </div>
          )}
      </div>
      {footer}
    </>
  );

  return (
    <div className={className} data-slot="command">
      <div className="command-input-wrapper" data-slot="command-input-wrapper">
        {icon}
        <input
          ref={(node) => {
            localInputRef.current = node;
            if (typeof inputRef === "function") inputRef(node);
            else if (inputRef !== undefined && inputRef !== null) (inputRef as { current: HTMLInputElement | null }).current = node;
          }}
          data-slot="command-input"
          className={inputClassName}
          placeholder={placeholder}
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          onKeyDown={onKeyDown}
          spellCheck={false}
          autoComplete="off"
          aria-autocomplete="list"
          role="combobox"
          aria-expanded
        />
      </div>
      {aside !== undefined ? (
        <div className={bodyClassName}>
          <div className={listColClassName}>{listColumn}</div>
          {aside}
        </div>
      ) : (
        listColumn
      )}
    </div>
  );
}
