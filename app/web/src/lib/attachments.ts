// Chat attachment support. The frontend reads a small text or
// source-code file through a Rust command and sends the text
// payload to `chat_send` along with the user message. We do not
// persist the attachment separately; the text is folded into the
// user message by the backend so the conversation history stays
// self-contained.

import { invoke } from "@tauri-apps/api/core";

export type AttachmentPayload = {
  /** Absolute path the user picked. */
  path: string;
  /** Filename only, for display. */
  filename: string;
  /** "code" | "text" | "unknown". */
  kind: string;
  /** MIME type reported by the extractor (or "text/plain"). */
  mime_type: string;
  /** File size in bytes on disk. */
  size_bytes: number;
  /** Text content (with the extractor-supplied prefix for code files). */
  text: string;
  /** True if the file was larger than the cap and we only kept the head. */
  truncated: boolean;
  /** True if the file contained non-UTF-8 bytes and we lossily replaced them. */
  non_utf8_replaced: boolean;
  /** Line count if known. */
  line_count: number | null;
};

export type AttachedText = {
  filename: string;
  kind: string;
  mime_type: string;
  text: string;
  /** Mirrors `AttachmentPayload.truncated`. */
  truncated: boolean;
};

export async function chatReadAttachment(path: string): Promise<AttachmentPayload> {
  return invoke<AttachmentPayload>("chat_read_attachment", { req: { path } });
}
