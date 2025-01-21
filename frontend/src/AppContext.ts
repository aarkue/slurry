import { time } from "console";
import { createContext } from "react";
import { z } from "zod";

export const connectionFormSchema = z.object({
  username: z.string().min(1, "Please enter a username."),
  host: z.tuple([
    z.string(),
    z
      .string()
      .transform((value) => (value === "" ? null : value))
      .nullable()
      .optional()
      .refine((value) => value === null || !isNaN(Number(value)), {
        message: "Invalid number",
      })
      .transform((value) => (value === null ? 22 : Number(value)))
      .or(z.number().int().positive()),
  ]),
  auth: z.discriminatedUnion("mode", [
    z.object({
      mode: z.literal("password-mfa"),
      password: z.string(),
      mfaCode: z.string(),
    }),
    z.object({
      mode: z.literal("ssh-key"),
      path: z.string(),
      passcode: z.string().optional(),
    }),
  ]),
});

export type SqueueRow = {account: string, state: string}
export type AppContextType = {
  runSqueue: () => Promise<string>;
  startSqueueLoop: (second_interval: number) => Promise<string>;
  stopSqueueLoop: () => Promise<string>,
  getLoopInfo: () => Promise<{secondInterval: number, runningSince: string, path: string}>,
  getSqueue: () => Promise<[string,SqueueRow[]]>,
  extractOCEL: () => Promise<string>;
  login: (cfg: z.infer<typeof connectionFormSchema>) => Promise<string>;
  logout: () => Promise<string>,
  isLoggedIn: () => Promise<boolean>,
  // Return unlisten function (to de-register)
  listenSqueue: (a: (timeAndRows: [string,SqueueRow[]]) => unknown) => Promise<() => unknown>,
  startTestJob: () => Promise<string>,
  checkJobStatus: (jobID: string) => Promise<{status: "PENDING", start_time: String|undefined} |{status: "RUNNING", start_time: String|undefined, end_time: String|undefined} | {status: "ENDED", state: string}  | {status: "NOT_FOUND"}>,
};

const throwNoContext = () => {
  throw new Error("No Context");
};
export const DEFAULT_NO_CONTEXT: AppContextType = {
  runSqueue: throwNoContext,
  getSqueue: throwNoContext,
  startSqueueLoop: throwNoContext,
  stopSqueueLoop: throwNoContext,
  getLoopInfo: throwNoContext,
  extractOCEL: throwNoContext,
  login: throwNoContext,
  logout: throwNoContext,
  isLoggedIn: throwNoContext,
  listenSqueue: throwNoContext,
  startTestJob: throwNoContext,
  checkJobStatus: throwNoContext
};
export const AppContext = createContext<AppContextType>(DEFAULT_NO_CONTEXT);
