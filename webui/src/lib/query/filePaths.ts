import { canRevealLocalPath, revealLocalPath } from "../api/filePaths";

export const localPathRuntime = { isDesktop: canRevealLocalPath, reveal: revealLocalPath };
