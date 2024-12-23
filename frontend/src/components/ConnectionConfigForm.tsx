import { connectionFormSchema } from "@/AppContext";
import { zodResolver } from "@hookform/resolvers/zod";
import { useCallback } from "react";
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

export default function ConnectionConfigForm({onSubmit, disabled}: {disabled?: boolean, onSubmit: (config: z.infer<typeof connectionFormSchema>) => any}) {
  // const { runSqueue: testSqueue } = useContext(AppContext);
  // const [isLoading, setIsLoading] = useState(false);
  // const [isPollingWith, setIsPollingWith] = useState<
  //   z.infer<typeof connectionFormSchema> | undefined
  // >(undefined);

  const form = useForm<z.infer<typeof connectionFormSchema>>({
    resolver: zodResolver(connectionFormSchema),
    defaultValues: {
      username: "",
      host: ["login23-1.hpc.itc.rwth-aachen.de", 22],
      auth: { mode: "password-mfa", password: "", mfaCode: "" },
    },
  });

  const submitCallback = useCallback(
    (values: z.infer<typeof connectionFormSchema>) => {
      onSubmit(values);
      // setIsLoading(true);
      // toast
      //   .promise(testSqueue(values), {
      //     loading: "Loading...",
      //     error: (e) => (
      //       <div>
      //         Failed!
      //         <br />
      //         {e.toString()}
      //       </div>
      //     ),
      //     success: (s) => (
      //       <div>
      //         Success!
      //         <br />
      //         {s}
      //       </div>
      //     ),
      //   })
      //   .finally(() => {
      //     setIsLoading(false);
      //   });
    },
    [],
  );

  // useEffect(() => {
  //   console.log(isPollingWith);
  //   let interval: NodeJS.Timeout | undefined;
  //   if (isPollingWith !== undefined) {
  //     onSubmit(isPollingWith);
  //     interval = setInterval(() => {
  //       onSubmit(isPollingWith);
  //     }, 1000 * 5);
  //   }
  //   return () => {
  //     if (interval != undefined) {
  //       clearInterval(interval);
  //     }
  //   };
  // }, [isPollingWith, form]);

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

            <FormField  disabled={disabled}
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

          <FormField  disabled={disabled}
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
                      <TabsTrigger  disabled={disabled}
                        value="password-mfa"
                      >
                        Password + MFA
                      </TabsTrigger>
                      <TabsTrigger  disabled={disabled}
                        value="ssh-key"
                      >
                        Private SSH Keyfile
                      </TabsTrigger>
                    </TabsList>
                    <div className="text-left ml-4">
                      <TabsContent value="password-mfa">
                        Login using a password and optionally a MFA token.
                        <FormField  disabled={disabled}
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
                        <FormField  disabled={disabled}
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
                        <FormField  disabled={disabled}
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
                        <FormField  disabled={disabled}
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
        </div>

        <Button disabled={disabled}
          type="submit"
        >
          Submit
        </Button>
        {/* <Button
          className="ml-2"
          variant={isPollingWith !== undefined ? "destructive" : "secondary"}
          onClick={(ev) => {
            if (isPollingWith == undefined) {
              form.handleSubmit(
                (d) => setIsPollingWith(d),
                (e) => console.error(e),
              )(ev);
            } else {
              setIsPollingWith(undefined);
            }
            ev.preventDefault();
            ev.stopPropagation();
          }}
        >
          {isPollingWith ? "End" : "Start"} Polling
        </Button> */}
      </form>
    </Form>
  );
}
