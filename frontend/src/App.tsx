import "@/globals.css";
import clsx from "clsx";
import { Folder, LogOut } from "lucide-react";
import { useCallback, useContext, useEffect, useState } from "react";
import toast, { Toaster } from "react-hot-toast";
import { AppContext, AppContextType } from "./AppContext";
import ConnectionConfigForm from "./components/ConnectionConfigForm";
import JobsOverview from "./components/JobsOverview";
import OCELExtractor from "./components/OCELExtractor";
import Spinner from "./components/ui/Spinner";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import { Label } from "./components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";

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
                <TabsTrigger value="job-creation" className="font-semibold">
                  Job Creation
                </TabsTrigger>
              </TabsList>
            </div>

            <TabsContent value="overview" className={clsx("!ring-opacity-5 h-full", "data-[state=inactive]:hidden")} forceMount>
              <JobsOverview />
            </TabsContent>
            <TabsContent value="data-collection">
              <DataCollection />
            </TabsContent>

            <TabsContent value="ocel-extraction">
              <OCELExtractor />
            </TabsContent>

            <TabsContent value="job-creation">
              <JobSubmission />
            </TabsContent>
          </Tabs>
        </div>}

      </main>
    </AppContext.Provider>
  );
}

function JobSubmission() {
  const [jobID, setJobID] = useState<string>();
  const [jobState, setJobState] = useState<{ status: "PENDING", start_time: String | undefined } | { status: "RUNNING", start_time: String | undefined, end_time: String | undefined } | { status: "ENDED", state: string } | { status: "NOT_FOUND" }>();
  const context = useContext(AppContext);

  useEffect(() => {
    if (jobID) {
      const t = setInterval(() => {
        // toast.promise(
        // )
        // , { loading: "Loading job status...", success: "Got job status", error: (e) => "Failed to get job status: " + e }
        context.checkJobStatus(jobID)
        .then((r) => setJobState(r))
      }, 3 * 1000);
      return () => {
        clearTimeout(t);
      }
    }
  }, [jobID])
  return <div className="flex flex-col gap-1 justify-center items-center">
    <Button disabled={jobID !== undefined} onClick={() => {
      toast.promise(context.startTestJob(), { loading: "Starting job...", success: (s) => "Started job: " + s, error: (e) => "Failed to start job: " + e }).then((j) => {
        setJobID(j);
      })
    }}>
      Create example job
    </Button>
    {jobID !== undefined && <p>
      Created job with ID {jobID}
    </p>}
   {jobState !== undefined && <div  className={clsx("block w-fit mx-auto p-2 rounded",{"PENDING": "bg-gray-300/20", "RUNNING": "bg-green-400/20", "ENDED": "bg-fuchsia-400/20", "NOT_FOUND": "bg-gray-100/20"}[jobState.status])}>
    <div className={clsx("block w-fit mx-auto p-2 rounded font-extrabold text-xl ",{"PENDING": "text-gray-500", "RUNNING": "text-green-500", "ENDED": "text-fuchsia-500", "NOT_FOUND": "text-gray-500"}[jobState.status])}>
      {jobState.status}
    </div>
    <div className="grid grid-cols-[auto,1fr] gap-x-1">
      {jobState.status === "RUNNING" && <>
        <span>Start:</span> <span>{jobState.start_time}</span>
        <span>End:</span> <span>{jobState.end_time}</span>
        </>}
        {jobState.status === "PENDING" && <>
        <span>Start:</span> <span>{jobState.start_time}</span>
        </>}

        {jobState.status === "ENDED" && <>
        <span>State:</span> <span>{jobState.state}</span>
        </>}
      
      </div>
    </div>}
    {jobID !== undefined && <Button onClick={() => { setJobID(undefined); setJobState(undefined) }} variant="destructive">
      Reset
    </Button>}
  </div>
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
          <span className="text-xs relative">to <Folder className="inline-block size-3 absolute top-1/2 -translate-y-1/2 ml-0.5 text-cyan-600" /> <input onClick={(ev) => {
            ev.currentTarget.select();
          }} value={loopInfo.path} className="border p-0.5 rounded pl-4 bg-cyan-50 border-cyan-200 text-xs" readOnly /></span><br />
          <span className="text-xs">since {new Date(loopInfo.runningSince).toLocaleString()}</span><br />
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