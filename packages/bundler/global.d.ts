import { NapiDiagnostic, NapiIssue } from "./src/binding";

declare global {
  export type TurbopackResult<T = {}> = T & {
    issues: NapiIssue[];
    diagnostics: NapiDiagnostic[];
  };
  export type RefCell = { readonly __tag: unique symbol };
  export type ExternalEndpoint = { readonly __tag: unique symbol };
}
