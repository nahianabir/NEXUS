// Typed IPC bindings for Document Intelligence. Phase J.

import { invoke } from "@tauri-apps/api/core";

export type ExtractedTable = {
  source_offset: number;
  headers: string[];
  rows: string[][];
};

export type CsvExportResponse = {
  table: ExtractedTable;
  body: string;
  markdown: string;
  row_count: number;
  column_count: number;
};

export type DiffLine = {
  kind: "equal" | "add" | "remove";
  text: string;
};

export type DiffSummary = {
  lines: DiffLine[];
  added: number;
  removed: number;
  same: number;
};

export async function documentExtractTables(text: string): Promise<ExtractedTable[]> {
  return invoke("document_extract_tables", { text });
}

export async function documentExportCsv(request: {
  text: string;
  index?: number | null;
  delimiter?: string | null;
  include_header?: boolean | null;
}): Promise<CsvExportResponse> {
  return invoke("document_export_csv", {
    request: {
      text: request.text,
      index: request.index ?? null,
      delimiter: request.delimiter ?? null,
      include_header: request.include_header ?? null,
    },
  });
}

export async function documentDiff(a: string, b: string): Promise<DiffSummary> {
  return invoke("document_diff", { a, b });
}
