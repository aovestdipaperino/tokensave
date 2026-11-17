export interface TsThing { value: number }

export function tsTarget(input: TsThing): number {
  return input.value;
}

export function firstTsCall(): number {
  return tsTarget({ value: 1 });
}

export function secondTsCall(input: TsThing): number {
  return tsTarget(input);
}
