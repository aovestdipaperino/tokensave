/**
 * Sample ESM (.mjs) file exercising the JavaScript grammar path (#219).
 */

import assert from 'node:assert/strict';
import test from 'node:test';

export const add = (a, b) => a + b;

test('adds two numbers', () => {
    assert.equal(add(1, 2), 3);
});
