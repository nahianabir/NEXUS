// A small hook that exposes a `prompt()`, `confirm()` and
// `alert()` function to any component, backed by the
// in-app `PromptDialog` modal mounted at body level.
//
// The hook is intentionally a no-op stub for state —
// all state is shared at module level by
// `components/PromptDialog.tsx`. Each call site just
// gets a thin async wrapper that returns a promise.
//
// Usage:
//   const { prompt, confirm, alert } = usePromptDialog();
//   const name = await prompt({ title: "Rename", label: "Name", defaultValue: "x" });
//   const ok = await confirm({ title: "Delete?", destructive: true });
//   await alert("Saved", "Your changes were saved.");
import { useCallback } from "react";
import { openPrompt, type PromptRequest } from "../components/PromptDialog";

type PromptOpts = Omit<PromptRequest, "mode">;
type ConfirmOpts = Omit<PromptRequest, "mode" | "validate">;

export function usePromptDialog() {
  const prompt = useCallback((opts: PromptOpts): Promise<string | null> => {
    return openPrompt("prompt", opts);
  }, []);

  const confirm = useCallback((opts: ConfirmOpts): Promise<boolean> => {
    return openPrompt("confirm", opts).then((v) => v === "ok");
  }, []);

  const alert = useCallback((title: string, description?: string): Promise<void> => {
    return openPrompt("alert", {
      title,
      description,
      submitLabel: "OK",
      cancelLabel: "Close",
    }).then(() => undefined);
  }, []);

  return { prompt, confirm, alert };
}
