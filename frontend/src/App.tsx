import "@/globals.css";
import { useState } from "react";
import toast, { Toaster } from "react-hot-toast";
import { AppContext, AppContextType } from "./AppContext";
import ConnectionConfigForm from "./components/ConnectionConfigForm";
import Spinner from "./components/ui/Spinner";
import { Button } from "./components/ui/button";
import { Circle, Dot, LogOut } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import OCELExtractor from "./components/OCELExtractor";
import JobsOverview from "./components/JobsOverview";

export default function App({ context }: { context: AppContextType }) {
  // TODO: Handle disconnects, ...
  const [loggedInStatus, setLoggedInStatus] = useState<'initial' | 'loading' | 'logged-in'>('initial');
  return (
    <AppContext.Provider value={context}>
      <main>
        <Toaster position="top-right" />
        {loggedInStatus !== 'logged-in' && <ConnectionConfigForm disabled={loggedInStatus !== 'initial'} onSubmit={(config) => {
          setLoggedInStatus('loading');
          toast.promise(context.login(config), { loading: "Logging In...", error: "Login failed!", success: "Login successful!" }).then(() => {
            setLoggedInStatus('logged-in')
          }).catch(() => {
            setLoggedInStatus('initial');
          })
        }} />}
        {loggedInStatus === 'loading' && <div className="flex justify-center">
          <Spinner className="w-8 h-8" />
        </div>
        }

        {loggedInStatus === 'logged-in' && <div className="mt-2 ml-2">
          <div className="flex items-center gap-x-1 justify-start ml-2 absolute left-0">
            <div className="size-4 rounded-full bg-green-600" />
            <p>Logged In</p>
            <Button size="icon" variant="ghost" className="ml-2" title="Log out" onClick={() => toast.promise(context.logout(), { loading: "Logging out...", error: "Failed to log out.", success: "Logged out!" }).finally(() => setLoggedInStatus("initial"))}><LogOut size={16} /></Button>
          </div>
          <Tabs className="mt-2" defaultValue="data-collection">
            <div className="text-center">
              <TabsList>
                <TabsTrigger value="overview" className="font-semibold">
                  Overview
                </TabsTrigger>
                <TabsTrigger value="data-collection" className="font-semibold">
                  Data Collection
                </TabsTrigger>
                <TabsTrigger value="ocel-extraction" className="font-semibold">
                  OCEL Extraction
                </TabsTrigger>
              </TabsList>
            </div>

            <TabsContent value="overview">
              <JobsOverview />
            </TabsContent>
            <TabsContent value="data-collection">
              <Button className="mx-auto block" onClick={() => {
                toast.promise(context.runSqueue(), { loading: "Running squeue....", error: "Failed to run squeue!", success: "Extracted squeue!" })
              }}>Pull</Button>
            </TabsContent>

            <TabsContent value="ocel-extraction">
              <OCELExtractor />
            </TabsContent>
          </Tabs>
        </div>}

      </main>
    </AppContext.Provider>
  );
}
