import { Config as AlphaConfig, helper as alphaHelper } from "./alpha";
import * as beta from "./beta";

export function run(): boolean {
  const a = AlphaConfig.create();
  const b = beta.Config.create();
  const c = new AlphaConfig(3);
  return a.check() && b.check() && c.check() && beta.helper() + alphaHelper() > 0;
}
