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
  getSqueue: () => Promise<[string,SqueueRow[]]>,
  extractOCEL: (data: [string, any][]) => Promise<string>;
  login: (cfg: z.infer<typeof connectionFormSchema>) => Promise<string>;
  logout: () => Promise<string>,
};

const throwNoContext = async () => {
  throw new Error("No Context");
};
export const DEFAULT_NO_CONTEXT: AppContextType = {
  runSqueue: throwNoContext,
  getSqueue: throwNoContext,
  extractOCEL: throwNoContext,
  login: throwNoContext,
  logout: throwNoContext,
};
export const AppContext = createContext<AppContextType>(DEFAULT_NO_CONTEXT);
