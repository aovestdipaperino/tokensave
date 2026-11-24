import { Validate } from "./validate";

export class Config implements Validate {
  constructor(public limit: number) {}

  static create(): Config {
    return new Config(1);
  }

  check(): boolean {
    return this.limit > 0;
  }
}

export function helper(): number {
  return 1;
}
