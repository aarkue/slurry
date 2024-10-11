import { z } from "zod";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Form, FormControl, FormDescription, FormField, FormItem, FormLabel, FormMessage } from "./ui/form";
import { Input } from "./ui/input";
import { Button } from "./ui/button";
import { AppContext, connectionFormSchema } from "@/AppContext";
import { useContext } from "react";
import toast from "react-hot-toast";


export default function ConnectionConfigForm() {
    const { testSqueue } = useContext(AppContext);

    const form = useForm<z.infer<typeof connectionFormSchema>>({
        resolver: zodResolver(connectionFormSchema),
        defaultValues: {
            username: "",
            host: ["login23-1.hpc.itc.rwth-aachen.de", 22],
            auth: { mode: "password-mfa", password: "", mfaCode: "" }
        },
    })


    function onSubmit(values: z.infer<typeof connectionFormSchema>) {
        console.log(values);
        toast.promise(testSqueue(values), { loading: "Loading...", error: (e) => <div>Failed!<br/>{e.toString()}</div>, success: s => <div>
            Success!
            <br/>
            {s}
        </div>});
    }

    return <Form {...form}><form className="mx-auto max-w-xl mt-4" onSubmit={form.handleSubmit(onSubmit)} >
        <div>
            <div className="flex gap-x-2">

                <FormField
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

                <FormField
                    control={form.control}
                    name="host.1"
                    render={({ field }) => (
                        <FormItem>
                            <FormLabel>SSH Port</FormLabel>
                            <FormControl>
                                <Input placeholder="22"  {...field} />
                            </FormControl>
                            <FormMessage />
                        </FormItem>
                    )}
                />
            </div>

            <FormField
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
                            <Tabs defaultValue="password-mfa" className="my-4" value={field.value.mode} onValueChange={(newMode) => {
                                if (newMode === "password-mfa") {
                                    const newVal: z.infer<typeof connectionFormSchema>['auth'] = { mode: "password-mfa", password: "", mfaCode: "" };
                                    field.onChange(newVal)
                                } else {
                                    const newVal: z.infer<typeof connectionFormSchema>['auth'] = { mode: "ssh-key", path: "" };
                                    field.onChange(newVal)
                                }
                            }}>
                                <TabsList>
                                    <TabsTrigger value="password-mfa">Password + MFA</TabsTrigger>
                                    <TabsTrigger value="ssh-key">Private SSH Keyfile</TabsTrigger>
                                </TabsList>
                                <div className="text-left ml-4">
                                    <TabsContent value="password-mfa">
                                        Login using a password and optionally a MFA token.
                                        <FormField
                                            control={form.control}
                                            name="auth.password"
                                            render={({ field }) => (
                                                <FormItem itemType="password">
                                                    <FormLabel>Password</FormLabel>
                                                    <FormControl>
                                                        <Input placeholder="" type="password" {...field} />
                                                    </FormControl>
                                                    <FormMessage />
                                                </FormItem>
                                            )}
                                        />
                                        <FormField
                                            control={form.control}
                                            name="auth.mfaCode"
                                            render={({ field }) => (
                                                <FormItem itemType="password">
                                                    <FormLabel>MFA Code</FormLabel>
                                                    <FormControl>
                                                        <Input placeholder="" type="password" {...field} />
                                                    </FormControl>
                                                    <FormMessage />
                                                </FormItem>
                                            )}
                                        />
                                    </TabsContent>
                                    <TabsContent value="ssh-key">
                                        Login using a private SSH key, optionally protected with an additional passcode.
                                        <FormField
                                            control={form.control}
                                            name="auth.path"
                                            render={({ field }) => (
                                                <FormItem >
                                                    <FormLabel>SSH Private Key Path</FormLabel>
                                                    <FormControl>
                                                        <Input placeholder="" {...field} />
                                                    </FormControl>
                                                    <FormMessage />
                                                </FormItem>
                                            )}
                                        />
                                        <FormField
                                            control={form.control}
                                            name="auth.passcode"
                                            render={({ field }) => (
                                                <FormItem itemType="password">
                                                    <FormLabel>Private Key Passcode (Optional)</FormLabel>
                                                    <FormControl>
                                                        <Input placeholder="" type="password" {...field} />
                                                    </FormControl>
                                                    <FormMessage />
                                                </FormItem>
                                            )}
                                        /></TabsContent>
                                </div>
                            </Tabs>
                        </FormControl>
                    </FormItem>
                )}
            />

        </div>

        <Button type="submit">Submit</Button>
    </form></Form>
}
