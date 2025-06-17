import { NapiDiagnostic, NapiIssue } from "./binding";

export { build } from "./build";
export { watch } from "./watch";

declare global {
  export type TurbopackResult<T = {}> = T & {
    issues: NapiIssue[];
    diagnostics: NapiDiagnostic[];
  };
  export type RefCell = { readonly __tag: unique symbol };
  export type ExternalEndpoint = { readonly __tag: unique symbol };
}
