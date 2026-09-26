// Strings for the app frame: sidebar, title bar, toast, dialogs (New session, Delete session), session form, folder field, CLI marks, status chips and the shared words in lib/format.ts. Keys start with "shell.". English stays word for word what the screens
// said before, so tests keep finding it; Indonesian must cover every key.
export const en = {} as const;

export const id: Record<keyof typeof en, string> = {};
