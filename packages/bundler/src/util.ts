import { bold, green, magenta, red } from "picocolors";
import { codeFrameColumns } from "@babel/code-frame";

import { NapiIssue } from "./binding";
import { StyledString } from "./types";
import {
  decodeMagicIdentifier,
  MAGIC_IDENTIFIER_REGEX,
} from "./magicIdentifier";

export function formatIssue(issue: NapiIssue) {
  const { filePath, title, description, source } = issue;
  let { documentationLink } = issue;
  let formattedTitle = renderStyledStringToErrorAnsi(title).replace(
    /\n/g,
    "\n    ",
  );

  let formattedFilePath = filePath
    .replace("[project]/", "./")
    .replaceAll("/./", "/")
    .replace("\\\\?\\", "");

  let message = "";

  if (source && source.range) {
    const { start } = source.range;
    message = `${formattedFilePath}:${start.line + 1}:${
      start.column + 1
    }\n${formattedTitle}`;
  } else if (formattedFilePath) {
    message = `${formattedFilePath}\n${formattedTitle}`;
  } else {
    message = formattedTitle;
  }
  message += "\n";

  if (source?.range && source.source.content) {
    const { start, end } = source.range;

    message +=
      codeFrameColumns(
        source.source.content,
        {
          start: {
            line: start.line + 1,
            column: start.column + 1,
          },
          end: {
            line: end.line + 1,
            column: end.column + 1,
          },
        },
        { forceColor: true },
      ).trim() + "\n\n";
  }

  if (description) {
    message += renderStyledStringToErrorAnsi(description) + "\n\n";
  }

  // TODO: make it possible to enable this for debugging, but not in tests.
  // if (detail) {
  //   message += renderStyledStringToErrorAnsi(detail) + '\n\n'
  // }

  // TODO: Include a trace from the issue.

  if (documentationLink) {
    message += documentationLink + "\n\n";
  }

  return message;
}

export function renderStyledStringToErrorAnsi(string: StyledString): string {
  function decodeMagicIdentifiers(str: string): string {
    return str.replaceAll(MAGIC_IDENTIFIER_REGEX, (ident) => {
      try {
        return magenta(`{${decodeMagicIdentifier(ident)}}`);
      } catch (e) {
        return magenta(`{${ident} (decoding failed: ${e})}`);
      }
    });
  }

  switch (string.type) {
    case "text":
      return decodeMagicIdentifiers(string.value);
    case "strong":
      return bold(red(decodeMagicIdentifiers(string.value)));
    case "code":
      return green(decodeMagicIdentifiers(string.value));
    case "line":
      return string.value.map(renderStyledStringToErrorAnsi).join("");
    case "stack":
      return string.value.map(renderStyledStringToErrorAnsi).join("\n");
    default:
      throw new Error("Unknown StyledString type", string);
  }
}
