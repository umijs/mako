import { getHeapSnapshot } from "node:v8";
import { describe, expect, it } from "vitest";
import type { NapiIssue } from "../binding";
import { DiagnosticSources } from "../core/diagnosticSources";

function diagnostic(content: string, ident = "source.js") {
  const issue: NapiIssue = {
    severity: "warning",
    stage: "resolve",
    filePath: ident,
    title: { type: "text", value: "Cannot resolve dynamic import" },
    documentationLink: "",
    importTraces: [],
    source: {
      source: { ident, content },
      range: {
        start: { line: 4, column: 2 },
        end: { line: 4, column: 8 },
      },
    },
  };
  return { issues: [issue] };
}

describe("diagnostic source text", () => {
  it("preserves diagnostics and edited content without sharing mutable source objects", () => {
    const sources = new DiagnosticSources();
    const first = diagnostic("before edit");
    const repeated = diagnostic("before edit");
    const changed = diagnostic("after edit");
    const expected = structuredClone([first, repeated, changed]);

    expect(sources.intern(first)).toBe(first);
    sources.intern(repeated);
    sources.intern(changed);

    expect([first, repeated, changed]).toEqual(expected);
    expect(first.issues[0].source?.source).not.toBe(
      repeated.issues[0].source?.source,
    );
    repeated.issues[0].source!.source.content = "caller mutation";
    expect(first.issues[0].source?.source.content).toBe("before edit");
    expect(changed.issues[0].source?.source.content).toBe("after edit");

    sources.dispose();
    sources.dispose();
    sources.clear();
    const late = diagnostic("after disposal");
    const lateBefore = structuredClone(late);
    expect(sources.intern(late)).toBe(late);
    expect(late).toEqual(lateBefore);
  });

  it("shares retained strings across result types while bounding cache entries and bytes", async () => {
    // String equality cannot detect whether V8 retained independent native string
    // allocations. Inspect their retained heap nodes to catch the memory regression.
    const prefix = "utoo-diagnostic-source-memory-regression-";
    const results: ReturnType<typeof diagnostic>[] = [];
    const contentLength = 100_000;
    const text = (name: string) => {
      const buffer = Buffer.alloc(contentLength, "x");
      buffer.write(prefix + name);
      return buffer.toString();
    };
    const add = (sources: DiagnosticSources, name: string, ident = name) => {
      const result = diagnostic(text(name), ident);
      results.push(result);
      sources.intern(result);
      return result;
    };

    const shared = new DiagnosticSources();
    const entrypoints = {
      issues: add(shared, "shared").issues,
      appPaths: [diagnostic(text("shared"), "shared")],
      libraryPaths: [diagnostic(text("shared"), "shared")],
    };
    results.push(...entrypoints.appPaths, ...entrypoints.libraryPaths);
    shared.intern(entrypoints);
    for (let i = 0; i < 20; i++) add(shared, "shared");

    const entries = new DiagnosticSources(1);
    add(entries, "entry-limit");
    add(entries, "other-entry");
    add(entries, "entry-limit");

    const bytes = new DiagnosticSources(256, contentLength * 3);
    add(bytes, "byte-limit");
    add(bytes, "other-byte");
    add(bytes, "byte-limit");

    const oversized = new DiagnosticSources(256, contentLength);
    add(oversized, "oversized");
    add(oversized, "oversized");

    const cleared = new DiagnosticSources();
    add(cleared, "cleared");
    cleared.clear();
    add(cleared, "cleared");

    add(new DiagnosticSources(), "separate-project");
    add(new DiagnosticSources(), "separate-project");

    const disposed = new DiagnosticSources();
    const transient = (name: string) => {
      disposed.intern(diagnostic(text(name), name));
    };
    transient("disposed-released");
    disposed.dispose();
    transient("disposed-late");
    add(disposed, "disposed-independent");
    add(disposed, "disposed-independent");

    const chunks: Buffer[] = [];
    for await (const chunk of getHeapSnapshot())
      chunks.push(Buffer.from(chunk));
    const heap = JSON.parse(Buffer.concat(chunks).toString()) as {
      snapshot: {
        meta: { node_fields: string[]; node_types: string[][] };
      };
      nodes: number[];
      strings: string[];
    };
    const fields = heap.snapshot.meta.node_fields;
    const width = fields.length;
    const type = fields.indexOf("type");
    const name = fields.indexOf("name");
    const size = fields.indexOf("self_size");
    const counts = new Map<string, number>();
    for (let i = 0; i < heap.nodes.length; i += width) {
      if (
        heap.snapshot.meta.node_types[0][heap.nodes[i + type]] !== "string" ||
        heap.nodes[i + size] < contentLength
      ) {
        continue;
      }
      const value = heap.strings[heap.nodes[i + name]];
      if (value.startsWith(prefix)) {
        const key = value.slice(prefix.length).replace(/x+$/, "");
        counts.set(key, (counts.get(key) || 0) + 1);
      }
    }

    expect(counts.get("shared")).toBe(1);
    expect(counts.get("entry-limit")).toBe(2);
    expect(counts.get("byte-limit")).toBe(2);
    expect(counts.get("oversized")).toBe(2);
    expect(counts.get("cleared")).toBe(2);
    expect(counts.get("separate-project")).toBe(2);
    expect.soft(counts.get("disposed-released") ?? 0).toBe(0);
    expect.soft(counts.get("disposed-late") ?? 0).toBe(0);
    expect.soft(counts.get("disposed-independent")).toBe(2);
    // Keep the disposed cache alive while proving it retains no late text.
    expect(disposed.intern(null)).toBeNull();
    // Keep the native-response-shaped results alive until after the snapshot.
    expect(results).toHaveLength(37);
  });
});
