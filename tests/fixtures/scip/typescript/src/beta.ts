import { Validate } from "./validate";
import * as alpha from "./alpha";

export class Config implements Validate {
  constructor(public name: string) {}

  static create(): Config {
    return new Config("b");
  }

  check(): boolean {
    return this.name.length > 0 && alpha.helper() > 0;
  }
}

export function helper(): number {
  return 2;
}
