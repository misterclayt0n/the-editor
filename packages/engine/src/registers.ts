export type RegisterData =
  | { readonly type: "text"; readonly fragments: readonly string[] }
  | { readonly type: "line"; readonly fragments: readonly string[] }
  | { readonly type: "block"; readonly rows: readonly string[] };

function freezeStrings(values: readonly string[]): readonly string[] {
  return Object.freeze([...values]);
}

function cloneRegisterData(data: RegisterData): RegisterData {
  switch (data.type) {
    case "text":
      return { type: "text", fragments: freezeStrings(data.fragments) };
    case "line":
      return { type: "line", fragments: freezeStrings(data.fragments) };
    case "block":
      return { type: "block", rows: freezeStrings(data.rows) };
  }
}

export const RegisterData = {
  text(fragments: readonly string[] | string): RegisterData {
    return {
      type: "text",
      fragments: freezeStrings(typeof fragments === "string" ? [fragments] : fragments),
    };
  },
  line(fragments: readonly string[] | string): RegisterData {
    return {
      type: "line",
      fragments: freezeStrings(typeof fragments === "string" ? [fragments] : fragments),
    };
  },
  block(rows: readonly string[]): RegisterData {
    return { type: "block", rows: freezeStrings(rows) };
  },
};

export function registerDataFragments(data: RegisterData): readonly string[] {
  return data.type === "block" ? data.rows : data.fragments;
}

export function firstRegisterFragment(data: RegisterData): string | undefined {
  return registerDataFragments(data)[0];
}

export function lastRegisterFragment(data: RegisterData): string | undefined {
  const fragments = registerDataFragments(data);
  return fragments[fragments.length - 1];
}

export type RegisterName = string;

export class RegisterError extends Error {}

export class InvalidRegisterNameError extends RegisterError {
  constructor(override readonly name: string) {
    super(`invalid register name ${JSON.stringify(name)}`);
  }
}

export function isValidRegisterName(name: string): boolean {
  return Array.from(name).length === 1;
}

function validateName(name: RegisterName): void {
  if (!isValidRegisterName(name)) throw new InvalidRegisterNameError(name);
}

export class Registers {
  private readonly inner = new Map<RegisterName, RegisterData>();

  static create(): Registers {
    return new Registers();
  }

  get size(): number {
    return this.inner.size;
  }

  get isEmpty(): boolean {
    return this.inner.size === 0;
  }

  has(name: RegisterName): boolean {
    validateName(name);
    return this.inner.has(name);
  }

  read(name: RegisterName): RegisterData | undefined {
    validateName(name);
    const data = this.inner.get(name);
    return data === undefined ? undefined : cloneRegisterData(data);
  }

  first(name: RegisterName): string | undefined {
    const data = this.read(name);
    return data === undefined ? undefined : firstRegisterFragment(data);
  }

  last(name: RegisterName): string | undefined {
    const data = this.read(name);
    return data === undefined ? undefined : lastRegisterFragment(data);
  }

  write(name: RegisterName, data: RegisterData): RegisterData | undefined {
    validateName(name);
    const previous = this.read(name);
    this.inner.set(name, cloneRegisterData(data));
    return previous;
  }

  push(name: RegisterName, value: string): void {
    validateName(name);
    const current = this.inner.get(name);
    if (current === undefined) {
      this.inner.set(name, RegisterData.text([value]));
      return;
    }
    if (current.type === "block")
      this.inner.set(name, RegisterData.block([...current.rows, value]));
    else if (current.type === "line")
      this.inner.set(name, RegisterData.line([...current.fragments, value]));
    else this.inner.set(name, RegisterData.text([...current.fragments, value]));
  }

  remove(name: RegisterName): RegisterData | undefined {
    validateName(name);
    const previous = this.read(name);
    this.inner.delete(name);
    return previous;
  }

  clear(): void {
    this.inner.clear();
  }

  *entries(): IterableIterator<readonly [RegisterName, RegisterData]> {
    for (const name of [...this.inner.keys()].sort()) {
      const data = this.inner.get(name);
      if (data !== undefined) yield [name, cloneRegisterData(data)] as const;
    }
  }
}
