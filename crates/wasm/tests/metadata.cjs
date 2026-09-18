// Run: crates/wasm/build.sh nodejs && node crates/wasm/tests/metadata.cjs
// An alternate wasm-bindgen nodejs module path may be supplied as the first argument.
const assert = require('node:assert/strict');
const path = require('node:path');
const { Covenant } = require(path.resolve(process.argv[2] || 'crates/wasm/pkg/smplx_wasm.js'));

for (const [method, args, pattern] of [
  ['commitmentMerkleRoot', [], /^[0-9a-f]{64}$/],
  ['scriptPubKeyHex', ['regtest'], /^5120[0-9a-f]{64}$/],
  ['scriptHash', ['regtest'], /^[0-9a-f]{64}$/],
  ['address', ['regtest'], /^ert1p/],
]) {
  const invalid = new Covenant('not valid simplicity');
  assert.throws(() => invalid[method](...args), error =>
    error instanceof Error && error.message.startsWith('Covenant does not compile: '));
  invalid.free();
  const valid = new Covenant('fn main() {}');
  assert.match(valid[method](...args), pattern);
  valid.free();
}
console.log('Covenant metadata: valid and invalid source passed for all four methods');
