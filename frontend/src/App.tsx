import "@/globals.css";
import { useCallback, useContext, useEffect, useState } from "react";
import toast, { Toaster } from "react-hot-toast";
import { AppContext, AppContextType } from "./AppContext";
import ConnectionConfigForm from "./components/ConnectionConfigForm";
import Spinner from "./components/ui/Spinner";
import { Button } from "./components/ui/button";
import { Circle, Dot, Folder, LogOut } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import OCELExtractor from "./components/OCELExtractor";
import JobsOverview from "./components/JobsOverview";
import { Input } from "./components/ui/input";
import { Label } from "./components/ui/label";
import clsx from "clsx";

let toastID: string | undefined = undefined
export default function App({ context }: { context: AppContextType }) {
  // TODO: Handle disconnects, ...
  const [loggedInStatus, setLoggedInStatus] = useState<'initial' | 'loading' | 'logged-in'>('loading');
  useEffect(() => {
    context.isLoggedIn().then((b) => {
      if (b) {
        if (!toastID) {
          toastID = toast.success("Already logged in!")
        }
        setLoggedInStatus("logged-in");
      } else {
        toastID = undefined;
        setLoggedInStatus("initial");
      }
    }).catch(e => {
      toastID = undefined;
      toast.error(String(e));
      setLoggedInStatus("initial");
    })
  }, [])
  return (
    <AppContext.Provider value={context}>
      <main className="h-screen">
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

        {loggedInStatus === 'logged-in' && <div className="mt-2 ml-2 h-full">
          <div className="flex items-center gap-x-1 justify-start ml-2 absolute left-0">
            <div className="size-4 rounded-full bg-green-600" />
            <p>Logged In</p>
            <Button size="icon" variant="ghost" className="ml-2" title="Log out" onClick={() => toast.promise(context.logout(), { loading: "Logging out...", error: "Failed to log out.", success: "Logged out!" }).finally(() => setLoggedInStatus("initial"))}><LogOut size={16} /></Button>
          </div>
          <Tabs defaultValue="data-collection" className="mt-2 h-full">
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

            <TabsContent value="overview" className={clsx("!ring-opacity-5 h-full","data-[state=inactive]:hidden")} forceMount>
              <JobsOverview />
            </TabsContent>
            <TabsContent value="data-collection">
              <DataCollection />
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


function DataCollection() {
  const context = useContext(AppContext);
  const [loopInfo, setLoopInfo] = useState<{ secondInterval: number, runningSince: string, path: string }>();
  const [loading, setLoading] = useState(false);
  const [loopConfig, setLoopConfig] = useState<{ secondInterval: number }>({ secondInterval: 5 });
  const updateLoopInfo = useCallback(() => {
    context.getLoopInfo().then((li) => {
      setLoopInfo(li)
    }).catch(() => {
      setLoopInfo(undefined);
    })
  }, [])
  useEffect(() => {
    updateLoopInfo()
  })
  return <div>
    <Button className="mx-auto block" disabled={loading} variant={"outline"} onClick={() => {
      setLoading(true);
      toast.promise(context.runSqueue(), { loading: "Running squeue....", error: "Failed to run squeue!", success: "Extracted squeue!" }).finally(() => setLoading(false))
    }}>Extract Once</Button>
    <br />
    {loopInfo === undefined &&
      <div className="flex flex-col items-center justify-center gap-1">
        <div className="flex flex-col items-center gap-0.5">
          <Label className="font-semibold">Second Interval</Label>
          <Input className="w-[14ch]" type="number" step={1} min={3} value={loopConfig.secondInterval} onChange={(ev) => setLoopConfig({ ...loopConfig, secondInterval: ev.currentTarget.valueAsNumber })} />
        </div>
        <Button className="mx-auto block" disabled={loading || loopInfo !== undefined} onClick={() => {
          if (Number.isInteger(loopConfig.secondInterval) && loopConfig.secondInterval >= 3) {

            setLoading(true);
            toast.promise(context.startSqueueLoop(loopConfig.secondInterval), { loading: "Starting loop....", error: "Failed to start loop!", success: "Started loop!" }).catch(e => console.error(e)).then(() => updateLoopInfo()).finally(() => setLoading(false))
          } else {
            toast("Please enter a valid interval of at least 3 seconds.");
          }
        }}>Start Recording</Button>
      </div>
    }
    {loopInfo !== undefined &&
      <div>
        <p className="text-center mb-2 leading-none">

          <span className="size-4 rounded-full bg-red-400 animate-pulse duration-1000 inline-block -mb-0.5 mr-1" />

          <span className="font-semibold text-lg">Recording</span><br />
          <span className="text-xs relative">to <Folder className="inline-block size-3 absolute top-1/2 -translate-y-1/2 ml-0.5 text-cyan-600"/> <input onClick={(ev) => {
            ev.currentTarget.select();
          }} value={loopInfo.path} className="border p-0.5 rounded pl-4 bg-cyan-50 border-cyan-200 text-xs" readOnly/></span><br/>
          <span className="text-xs">since {new Date(loopInfo.runningSince).toLocaleString()}</span><br/>
          <span className="text-xs">with {loopInfo.secondInterval}s breaks.</span>
        </p>
        <Button variant="destructive" className="mx-auto block" disabled={loading || loopInfo === undefined} onClick={() => {
          setLoading(true);
          toast.promise(context.stopSqueueLoop(), { loading: "Stopping loop....", error: "Failed to stop loop!", success: "Stopped loop!" }).catch(e => console.error(e)).then(() => updateLoopInfo()).finally(() => setLoading(false))
        }}>Stop Recording</Button>
      </div>
    }
  </div>
}