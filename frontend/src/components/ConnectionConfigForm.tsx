import { connectionFormSchema } from "@/AppContext";
import { zodResolver } from "@hookform/resolvers/zod";
import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { Button } from "./ui/button";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage
} from "./ui/form";
import { Input } from "./ui/input";
import { InputOTP, InputOTPGroup, InputOTPSlot } from "./ui/input-otp";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { Checkbox } from "./ui/checkbox";
import { Label } from "./ui/label";
const SAVED_AUTH_LOCAL_STORAGE_KEY = "saved-auth";
export default function ConnectionConfigForm({ onSubmit, disabled }: { disabled?: boolean, onSubmit: (config: z.infer<typeof connectionFormSchema>) => any }) {
  const [saveLoginInfo, setSaveLoginInfo] = useState(false);
  const form = useForm<z.infer<typeof connectionFormSchema>>({
    resolver: zodResolver(connectionFormSchema),
    defaultValues: {
      username: "",
      host: ["login23-1.hpc.itc.rwth-aachen.de", 22],
      auth: { mode: "password-mfa", password: "", mfaCode: "" },
    },
  });

  useEffect(() => {
    const s = localStorage.getItem(SAVED_AUTH_LOCAL_STORAGE_KEY);
    if(s != null){
      try{
        let l: z.infer<typeof connectionFormSchema> = JSON.parse(s);
        console.log("Got login info",l);
        form.reset(l);
      }catch(e){
        console.error("Failed to parse saved login data")
      }
    }
  },[])

  const submitCallback = useCallback(
    (values: z.infer<typeof connectionFormSchema>) => {
      try {
        console.log(saveLoginInfo)
        if (saveLoginInfo) {
          const saveCopy = structuredClone(values);
          if (saveCopy.auth.mode === "password-mfa") {
            saveCopy.auth.mfaCode = "";
          }
          localStorage.setItem(SAVED_AUTH_LOCAL_STORAGE_KEY, JSON.stringify(saveCopy))
        }
      } catch (e) {
        console.error("Failed to save login data: " + String(e));
      } finally {
        onSubmit(values);
      }
    },
    [],
  );

  return (
    <Form {...form}>
      <form
        className="mx-auto max-w-xl mt-4"
        onSubmit={form.handleSubmit(submitCallback)}
      >
        <div>
          <div className="flex gap-x-2">
            <FormField disabled={disabled}
              control={form.control}
              name="host.0"
              render={({ field }) => (
                <FormItem className="w-full">
                  <FormLabel>SSH Host</FormLabel>
                  <FormControl>
                    <Input placeholder="ab12345" {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

            <FormField disabled={disabled}
              control={form.control}
              name="host.1"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>SSH Port</FormLabel>
                  <FormControl>
                    <Input placeholder="22" {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
          </div>

          <FormField disabled={disabled}
            control={form.control}
            name="username"
            render={({ field }) => (
              <FormItem>
                <FormLabel>Username</FormLabel>
                <FormControl>
                  <Input placeholder="ab12345" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="auth"
            render={({ field }) => (
              <FormItem>
                <FormControl>
                  <Tabs
                    defaultValue="password-mfa"
                    className="my-4"
                    value={field.value.mode}
                    onValueChange={(newMode) => {
                      if (newMode === "password-mfa") {
                        const newVal: z.infer<
                          typeof connectionFormSchema
                        >["auth"] = {
                          mode: "password-mfa",
                          password: "",
                          mfaCode: "",
                        };
                        field.onChange(newVal);
                      } else {
                        const newVal: z.infer<
                          typeof connectionFormSchema
                        >["auth"] = { mode: "ssh-key", path: "" };
                        field.onChange(newVal);
                      }
                    }}
                  >
                    <TabsList>
                      <TabsTrigger disabled={disabled}
                        value="password-mfa"
                      >
                        Password + MFA
                      </TabsTrigger>
                      <TabsTrigger disabled={disabled}
                        value="ssh-key"
                      >
                        Private SSH Keyfile
                      </TabsTrigger>
                    </TabsList>
                    <div className="text-left ml-4">
                      <TabsContent value="password-mfa">
                        Login using a password and optionally a MFA token.
                        <FormField disabled={disabled}
                          control={form.control}
                          name="auth.password"
                          render={({ field }) => (
                            <FormItem itemType="password">
                              <FormLabel>Password</FormLabel>
                              <FormControl>
                                <Input
                                  placeholder=""
                                  type="password"
                                  {...field}
                                />
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                        <FormField disabled={disabled}
                          control={form.control}
                          name="auth.mfaCode"
                          render={({ field }) => (
                            <FormItem itemType="password">
                              <FormLabel>MFA Code</FormLabel>
                              <FormControl>
                                <InputOTP maxLength={6} {...field}>
                                  <InputOTPGroup>
                                    <InputOTPSlot index={0} />
                                    <InputOTPSlot index={1} />
                                    <InputOTPSlot index={2} />
                                    <InputOTPSlot index={3} />
                                    <InputOTPSlot index={4} />
                                    <InputOTPSlot index={5} />
                                  </InputOTPGroup>
                                </InputOTP>
                                {/* <Input placeholder="" type="password" {...field} /> */}
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                      </TabsContent>
                      <TabsContent value="ssh-key">
                        Login using a private SSH key, optionally protected with
                        an additional passcode.
                        <FormField disabled={disabled}
                          control={form.control}
                          name="auth.path"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>SSH Private Key Path</FormLabel>
                              <FormControl>
                                <Input placeholder="" {...field} />
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                        <FormField disabled={disabled}
                          control={form.control}
                          name="auth.passcode"
                          render={({ field }) => (
                            <FormItem itemType="password">
                              <FormLabel>
                                Private Key Passcode (Optional)
                              </FormLabel>
                              <FormControl>
                                <Input
                                  placeholder=""
                                  type="password"
                                  {...field}
                                />
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                      </TabsContent>
                    </div>
                  </Tabs>
                </FormControl>
              </FormItem>
            )}
          />
          <Label className="flex items-center mb-2">
            <Checkbox className="mr-1" checked={saveLoginInfo} onCheckedChange={(c) => setSaveLoginInfo(c === true)} />
            Save login info
          </Label>
        </div>

        <Button disabled={disabled}
          type="submit"
        >
          Submit
        </Button>
      </form>
    </Form>
  );
}
