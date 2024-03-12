import * as import_wildcard from "ignored";
import { some as import_partial } from "ignored";
import import_default from "ignored";

it("ignored module should compile to empty es module", () => {
  expect(import_wildcard).toStrictEqual({})
})

it("ignored module should not export anything", () => {
  expect(import_partial).toBe(undefined)
})

it("ignored module should not have a valued export default", () => {
  expect(import_default).toBe(undefined)
})

