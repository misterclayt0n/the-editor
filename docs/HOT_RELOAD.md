# File Hot Reload and Workspace Watching

This document defines the target architecture for file hot reload in the editor.

It answers six questions:

1. What is being reloaded?
2. Which layer owns filesystem watching?
3. How do file watches update open documents?
4. How do file watches update the file tree and future workspace features?
5. How are dirty/conflicted documents represented?
6. Which parts are runtime state versus TypeScript product behavior?

This document is about **external file changes on disk**. It is not about TypeScript module reload, extension reload, package reload, or renderer UI module reload.

---

## Scope

File hot reload means:

```txt
external process changes a file on disk
→ the runtime notices through the workspace/file watcher
→ open clean documents reload from disk through normal transactions
→ open dirty documents become conflicted instead of being overwritten
→ workspace consumers such as the file tree observe the same underlying file changes
```

The goal is a single coherent file-watching architecture. The file tree, open-document hot reload, later search invalidation, later git/VCS refresh, and later LSP watched-file notifications should all consume the same runtime-owned filesystem truth rather than inventing separate watchers.

---

## North Star

The design should be Zed-like:

```txt
runtime workspace watcher
  → workspace snapshot of files/directories/mtimes
  → changed entry batches
  → resource/document disk-state updates
  → clean-document reload or dirty-document conflict
  → runtime events for TypeScript and renderer invalidation
```

Layer ownership:

```txt
the-lib
  pure Document text state and transaction application

the-runtime
  workspace roots, file watchers, workspace snapshots, resource disk state,
  open-document reload/conflict state, save/reload baselines

TypeScript product layer
  file tree UI state, tab indicators, save/close prompts, commands, policy hooks

Renderer
  paints tabs, file tree, prompts, and invalidated text surfaces
```

`the-runtime` owns the impure filesystem machinery. TypeScript consumes derived runtime state and decides how the default product presents it.

---

## Runtime workspace model

`the-runtime` should own a multi-root-capable workspace model.

V1 may use one root, the current `cwd`, but the runtime model should not bake in single-root assumptions.

Preferred shape:

```txt
WorkspaceRootId
WorkspaceEntryId
WorkspaceRoot
WorkspaceEntry
WorkspaceSnapshot
WorkspaceEntriesChanged
```

A workspace root owns:

- canonical absolute root path
- recursive file/directory snapshot
- stable entry IDs where practical
- normalized relative paths
- file kind
- mtime/disk metadata
- watcher task/subscription
- scan/reconcile state
- monotonically increasing snapshot revision

Canonical snapshot projection should be a flat entry list, not a UI tree:

```ts
type WorkspaceEntry = {
  id: WorkspaceEntryId;
  rootId: WorkspaceRootId;
  path: string; // normalized relative path
  kind: "file" | "directory" | "symlink";
  mtime?: number;
};
```

TypeScript may project this flat list into a tree for rendering.

### Why a snapshot instead of ad hoc events

Raw watcher events are platform-noisy and incomplete. A snapshot gives the runtime an authoritative answer to:

- does this path currently exist?
- is it a file or directory?
- what is its current mtime?
- which open document/resource is attached to it?
- which file tree entries changed?

This is the same broad design as Zed's worktree model: watchers update workspace state; higher layers consume changed entries.

---

## Watched units

The runtime should watch two categories of paths.

### 1. Workspace roots

Workspace roots are watched recursively and maintain workspace snapshots. The default file tree consumes these snapshots.

### 2. Open resources outside workspace roots

If a user opens a file outside every workspace root, that file should still participate in hot reload. The runtime should add an explicit one-file watch for that open resource.

Outside-root resources must not implicitly become file tree roots.

```txt
workspace roots
  → file tree + hot reload + later workspace services

outside-root open resource watches
  → hot reload only
```

Both categories should feed the same internal resource disk-state update path.

---

## Runtime document/resource state

`Document` in `the-lib` remains pure text truth. Runtime file state belongs in the runtime document/resource entry.

The runtime should track a structured save baseline and disk state, not a bag of booleans.

Target shape:

```rust
pub struct RuntimeDocument {
  pub document: Document,
  pub resource: Option<ResourceId>,
  pub save_baseline: Option<SaveBaseline>,
  pub readonly: bool,
  pub external_change: ExternalChangeState,
}

pub struct SaveBaseline {
  pub revision: u64,
  pub mtime: Option<FileMtime>,
}

pub enum DiskState {
  New,
  Present { mtime: FileMtime },
  Deleted,
  Unknown,
}

pub enum ExternalChangeState {
  None,
  ChangedOnDisk,
  DeletedOnDisk,
}
```

Public status should be derived:

```txt
dirty = save_baseline.revision != document.revision()
conflict = external_change != None && dirty
deletedOnDisk = external_change == DeletedOnDisk
externalChangePending = external_change != None
```

A TypeScript snapshot may expose:

```ts
type ResourceDocumentStatus = {
  dirty: boolean;
  conflict: boolean;
  deletedOnDisk: boolean;
  externalChangePending: boolean;
  readonly: boolean;
  diskState: "new" | "present" | "deleted" | "unknown";
};
```

---

## Save and reload baselines

Follow Zed's baseline model.

When opening a file:

```txt
read file text
stat/read mtime
create Document
save_baseline = { revision: document.revision(), mtime }
external_change = None
```

When saving a file:

```txt
write document text
stat/read resulting mtime
save_baseline = { revision: document.revision(), mtime }
external_change = None
```

When reloading from disk:

```txt
read disk text
compute diff against current document text
apply diff as a normal transaction
stat/read mtime
save_baseline = { revision: document.revision(), mtime }
external_change = None
```

There should be no timeout-based suppression of "our own save" watcher events. A later watcher event is interpreted against the stored save baseline. If disk metadata matches the baseline, it is already accounted for.

---

## External change handling

When a workspace changed-entry batch or outside-root file watcher reports a changed path, the runtime should:

1. normalize/canonicalize the path
2. update workspace/resource disk state
3. find the attached open document, if any
4. compare disk metadata against the document save baseline
5. choose reload or conflict behavior
6. emit derived runtime events

### Clean open document

If the document is clean and the file changed on disk:

```txt
dirty == false
external file state changed
file is present/readable
→ reload from disk automatically
```

The reload must be diff/transaction-based. Full text replacement is not an acceptable shortcut. If `the-lib` lacks the right diff-to-transaction primitive, add it.

The runtime should emit something like:

```ts
{
  type: "resource.reloaded",
  documentId,
  resourceId,
  path,
  oldRevision,
  newRevision
}
```

And invalidate all surfaces attached to the document:

```ts
{
  type: "surface.invalidated",
  surfaceIds,
  reason: "reload"
}
```

### Dirty open document

If the document has unsaved edits and the file changed on disk:

```txt
dirty == true
external file state changed
→ do not overwrite editor text
→ mark external_change
→ expose conflict status
```

The runtime should emit something like:

```ts
{
  type: "resource.reloadConflict",
  documentId,
  resourceId,
  path
}
```

The invariant is:

```txt
watcher events never overwrite dirty document text
```

### Deleted file

If a backing file is deleted externally:

- clean documents should become deleted-on-disk resources while keeping the text visible
- dirty documents should become conflicted/deleted-on-disk
- TypeScript decides how to prompt on save/close

---

## Reload transactions and history

External hot reload should create a normal recorded transaction, following Zed's behavior.

That means:

```txt
external clean-file reload
→ diff transaction is applied
→ transaction participates in undo/redo history
→ undoing the reload makes the document dirty relative to disk
```

The runtime may eventually expose reload history policy for explicit commands, but watcher-driven reload should record the transaction by default.

---

## Conflict save/close behavior

The runtime is UI-free. It should expose conflict state and typed save/reload operations; TypeScript owns prompts.

Runtime save behavior:

```txt
save(document)
  if conflict and not overwrite:
    return typed ResourceConflict error
  else:
    write file and update baseline
```

Runtime should expose explicit force operations such as:

```ts
runtime.resources.save({ documentId, conflict: "overwrite" })
runtime.resources.reload({ documentId })
```

The default distribution should implement Zed-like prompts through composable hooks/providers.

For a conflicted dirty file on save:

```txt
This file has changed on disk since you started editing it. Do you want to overwrite it?
[Overwrite] [Discard] [Cancel]
```

- Overwrite: force save editor contents to disk
- Discard: reload from disk
- Cancel: do nothing

For a dirty file deleted on disk:

```txt
This file has been deleted on disk since you started editing it. Do you want to recreate it?
[Save] [Close] [Cancel]
```

For a dirty non-conflicted file on close:

```txt
Save changes?
[Save] [Don't Save] [Cancel]
```

---

## Composable TypeScript presentation

Dirty/conflict UI must not be hardcoded into the runtime. It should be exposed through editor substrate hooks/providers so alternate distributions can present it differently.

The default distribution should register Zed-like behavior:

```txt
dirty tab indicator      → accent dot
conflict tab indicator   → warning dot
file tree dirty/conflict → optional status decoration
save conflict            → Overwrite / Discard / Cancel prompt
close dirty/conflict     → Save/Overwrite/Discard/Cancel prompt family
```

Possible substrate seams:

```ts
editor.tabs.registerIndicatorProvider(...)
editor.resources.registerStatusProvider(...)
editor.resources.registerConflictHandler(...)
editor.workbench.registerCloseGuard(...)
```

The important split:

```txt
Runtime owns the truth: dirty/conflict/deleted-on-disk.
TypeScript owns presentation and workflow composition.
```

---

## File tree integration

The current default file tree recursively reads paths with Node APIs. That should become a consumer of the runtime workspace snapshot.

Runtime owns:

- workspace entry existence
- file/directory kind
- mtime metadata
- ignored/excluded path policy
- changed entry batches
- rescan fallback

TypeScript file tree owns:

- expanded/collapsed UI state
- selected/active path
- rendering
- commands such as create/rename/delete/open
- optional sorting/grouping presentation policy

Default file tree flow:

```txt
runtime.workspace.snapshot(rootId)
  → default file tree stores flat entries + revision
  → renderer projects entries into tree UI

runtime event: workspace.entriesChanged
  → default file tree patches/refetches snapshot
  → workbench invalidates tools
```

File operations initiated by the file tree should go through runtime workspace/resource operations over time, not direct Node `fs` calls, so the runtime can keep the snapshot and resource mappings coherent.

---

## Public event shape

Raw watcher events should be diagnostic-only. Product code should consume derived domain events.

Public/default events:

```ts
type RuntimeFileDomainEvent =
  | { type: "workspace.entriesChanged"; rootId: WorkspaceRootId; revision: number; changes: WorkspaceEntryChange[] }
  | { type: "workspace.rootRescanned"; rootId: WorkspaceRootId; revision: number }
  | { type: "resource.reloaded"; documentId: DocumentId; resourceId: ResourceId; path: string; oldRevision: number; newRevision: number }
  | { type: "resource.reloadConflict"; documentId: DocumentId; resourceId: ResourceId; path: string }
  | { type: "resource.deletedOnDisk"; documentId: DocumentId; resourceId: ResourceId; path: string }
  | { type: "surface.invalidated"; surfaceIds: SurfaceId[]; reason: "reload" | "resource" };
```

Diagnostic events may include raw/normalized watcher batches and watch errors, but they should not be the default product interface.

---

## Implementation order

Recommended sequence:

1. Add runtime disk metadata/baseline state to resource-backed documents.
2. Add diff-to-transaction reload support in `the-lib`/`the-runtime` if missing.
3. Add runtime workspace root model and initial snapshot scan.
4. Add watcher reconciliation and changed-entry events.
5. Wire workspace changed entries to resource/open-document reload/conflict handling.
6. Add runtime event delivery through `packages/runtime-node` and Electron main/policy bridge.
7. Move default file tree from direct recursive `readdir` to runtime workspace snapshot.
8. Add resource status snapshots to workbench tabs/surface panes.
9. Add default dirty/conflict tab indicators and save/close conflict prompts through composable TS seams.
10. Add outside-root open resource one-file watches.

---

## Non-goals

This document does not cover:

- TypeScript module hot reload
- extension/package reload
- renderer UI module reload
- LSP watched-file notifications, except as a future consumer
- git status computation, except as a future consumer
- merge conflict marker parsing
- automatic merging of dirty editor text with external disk edits

Automatic merge is a separate future Module. V1 should mark conflict and preserve user text.
