import { toDisposable, type Disposable } from "./disposable.js";

export type ModuleId = string & { readonly __brand: "ModuleId" };
export type ContributionKind = string & { readonly __brand?: "ContributionKind" };

export interface ContributionId {
  moduleId: ModuleId;
  kind: ContributionKind;
  localId: string;
}

export interface Contribution {
  id: ContributionId;
  label?: string;
  disposable: Disposable;
}

export interface ContributionInput {
  moduleId: ModuleId;
  kind: ContributionKind;
  localId: string;
  label?: string;
  disposable: Disposable;
}

export class ContributionRegistry {
  private readonly contributions = new Map<string, Contribution>();

  add(input: ContributionInput): Disposable {
    const key = contributionKey(input);
    const existing = this.contributions.get(key);
    if (existing) existing.disposable.dispose();

    const contribution: Contribution = {
      id: {
        moduleId: input.moduleId,
        kind: input.kind,
        localId: input.localId,
      },
      label: input.label,
      disposable: input.disposable,
    };
    this.contributions.set(key, contribution);

    return toDisposable(() => {
      const current = this.contributions.get(key);
      if (current !== contribution) return;
      this.contributions.delete(key);
      contribution.disposable.dispose();
    });
  }

  byModule(moduleId: ModuleId): Contribution[] {
    return [...this.contributions.values()].filter((contribution) => contribution.id.moduleId === moduleId);
  }

  all(): Contribution[] {
    return [...this.contributions.values()];
  }

  disposeModule(moduleId: ModuleId): void {
    for (const contribution of this.byModule(moduleId)) {
      contribution.disposable.dispose();
      this.contributions.delete(contributionKey(contribution.id));
    }
  }
}

export function contributionKey(id: ContributionId): string {
  return `${id.moduleId}:${id.kind}:${id.localId}`;
}

export function moduleId(id: string): ModuleId {
  return id as ModuleId;
}

export function contributionKind(kind: string): ContributionKind {
  return kind as ContributionKind;
}
