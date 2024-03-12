import assert from "assert";
import * as import_wildcard from "ignored";
import { some as import_partial } from "ignored";
import import_default from "ignored";

assert.deepEqual(import_wildcard, {}, `ignored module should compile to empty es module`)
assert.equal(import_partial, undefined, `ignored module should not export anything`)
assert.equal(import_default, undefined, `ignored module should not have a valued export default`)
