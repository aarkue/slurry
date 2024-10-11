import { createContext } from "react";
import { z } from "zod";

export const connectionFormSchema = z.object({
    username: z.string().min(1, "Please enter a username."), host: z.tuple([z.string(), z.string()
        .transform((value) => (value === '' ? null : value))
        .nullable().optional()
        .refine((value) => value === null || !isNaN(Number(value)), {
            message: 'Invalid number',
        })
        .transform((value) => (value === null ? 22 : Number(value))).or(z.number().int().positive())]), auth: z.discriminatedUnion("mode", [z.object({ mode: z.literal("password-mfa"), password: z.string(), mfaCode: z.string() }), z.object({ mode: z.literal("ssh-key"), path: z.string(), passcode: z.string().optional() }),])
});

export type AppContextType = {
    "testSqueue": (cfg: z.infer<typeof connectionFormSchema>) => Promise<string>,
};
export const AppContext = createContext<AppContextType>({
    "testSqueue": async (cfg: z.infer<typeof connectionFormSchema>) => {
        throw new Error("No Context")
    },
})