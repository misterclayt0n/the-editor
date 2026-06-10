import { Data } from "effect";
import type { PanelId, SlotId } from "./ids.js";

export class UnknownPanelInArrangementError extends Data.TaggedError(
  "UnknownPanelInArrangementError",
)<{ readonly panelId: PanelId }> {}

export class DuplicateSlotIdError extends Data.TaggedError("DuplicateSlotIdError")<{
  readonly slotId: SlotId;
}> {}

export class DuplicatePanelInStackError extends Data.TaggedError("DuplicatePanelInStackError")<{
  readonly slotId: SlotId;
  readonly panelId: PanelId;
}> {}

export class InvalidWeightError extends Data.TaggedError("InvalidWeightError")<{
  readonly weight: number;
}> {}

export type ArrangementError =
  | UnknownPanelInArrangementError
  | DuplicateSlotIdError
  | DuplicatePanelInStackError
  | InvalidWeightError;

export class PanelNotFoundError extends Data.TaggedError("PanelNotFoundError")<{
  readonly panelId: PanelId;
}> {}

export class SlotNotFoundError extends Data.TaggedError("SlotNotFoundError")<{
  readonly slotId: SlotId;
}> {}
