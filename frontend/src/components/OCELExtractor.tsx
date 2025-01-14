import clsx from "clsx";
import { CalendarIcon, XIcon } from "lucide-react";
import { useContext, useState } from "react";
import DropZone from "./DropZone";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import toast from "react-hot-toast";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { AppContext } from "@/AppContext";
type FileWithInfo = { file: File; timestamp: string | null };

export default function OCELExtractor() {
  // const [files, setFiles] = useState<FileWithInfo[]>([]);
  // const [editFileDialog, setEditFileDialog] = useState<{
  //   info: FileWithInfo;
  //   index: number;
  // }>();
  const [loading, setLoading] = useState(false);
  const backend = useContext(AppContext);
  return (
    <div className="text-center">
      <Button onClick={() => {
        toast.promise(backend.extractOCEL(),{
          loading: "Extracting...",
          success: (s) => s,
          error: (e) => `Failed to extract: ${String(e)}`
        });
      }}>
        Extract OCEL
      </Button>
      {/* <DropZone
        onFilesAdded={(newFiles) => {
          setFiles((fs) => [
            ...fs,
            ...newFiles.map((f) => {
              try {
                const t = new Date(f.name.replace(".json", "").replace(/(T.*)$/,(s,a,b) => a.replaceAll("-",":"))).toISOString();
                return { file: f, timestamp: t };
              } catch (e) {
                return { file: f, timestamp: null };
              }
            }),
          ]);
        }}
      /> */}
      {/* <div className="max-w-lg mx-auto my-2">
        <div className="flex justify-between">
          <h3 className="text-xl font-semibold">Selected Files</h3>
          <Button
            variant="destructive"
            size="sm"
            disabled={files.length == 0}
            onClick={() => setFiles([])}
          >
            Clear All
          </Button>
        </div>
        {files.length === 0 && <span>No files selected.</span>}
        <ol className="[&>li]:list-disc pl-8">
          {files.map((f, i) => (
            <li
              key={i}
              className="grid grid-cols-[1rem,21rem,1rem] items-center gap-x-1"
            >
              <button
                className="size-4 text-red-300 hover:bg-red-100 hover:text-red-600 rounded"
                onClick={() => {
                  const newFiles = [...files];
                  newFiles.splice(i, 1);
                  setFiles(newFiles);
                }}
              >
                <XIcon className="size-4" />
              </button>
              {f.file.name}
              <button
                onClick={() => setEditFileDialog({ info: { ...f }, index: i })}
                className={clsx(
                  "size-4 rounded hover:text-black",
                  f.timestamp == null && "text-orange-400",
                  f.timestamp !== null && "text-green-400",
                )}
                title={
                  f.timestamp === null
                    ? "Not timestamp detected. Click to set a timestamp manually."
                    : "Timestamp detected: " +
                      f.timestamp +
                      ". Click to change."
                }
              >
                <CalendarIcon className="size-4" />
              </button>
            </li>
          ))}
        </ol>
        <Button
          className="mx-auto block mt-2"
          size="lg"
          disabled={loading}
          onClick={async () => {
            setLoading(true);
            const resPromise = Promise.allSettled(
              files.map((f) =>
                f.file.text().then((t) => [f.timestamp, JSON.parse(t)]),
              ),
            );

            const res = await toast.promise(resPromise, {
              loading: "Gathering JSON...",
              error: "Failed to gather files.",
              success: "Gathered JSON files.",
            });
            const jsonContents = res
              .filter((r) => r.status === "fulfilled")
              .map((r) => r.value);
            if (jsonContents.length === res.length) {
              toast.success(`Successfully parsed all files as JSON.`);
            } else {
              toast(
                `Successfully parsed ${jsonContents.length}/${res.length} files as JSON.`,
              );
            }
            try {
              const s = await toast.promise(
                backend.extractOCEL(jsonContents as any),
                {
                  loading: "Extracting OCEL...",
                  error: "Failed to extract OCEL.",
                  success: "Sucessfully extracted OCEL.",
                },
              );
              toast(s);
            } catch (e) {
              console.error(e);
            }

            setLoading(false);
          }}
        >
          Submit
        </Button>
      </div>
      <Dialog
        open={editFileDialog !== undefined}
        onOpenChange={(o) => {
          if (!o) {
            setEditFileDialog(undefined);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Edit File Information</DialogTitle>
          </DialogHeader>
          <div>
            <Label>Timestamp</Label>
            <Input
              defaultValue={editFileDialog?.info.timestamp ?? undefined}
              onChange={(ev) => {
                const timestamp = ev.currentTarget.value;
                setEditFileDialog({
                  ...editFileDialog!,
                  info: { ...editFileDialog!.info, timestamp },
                });
              }}
            />
          </div>

          <DialogFooter>
            <Button
              type="submit"
              variant="secondary"
              onClick={() => setEditFileDialog(undefined)}
            >
              Close
            </Button>
            <Button
              type="submit"
              onClick={(ev) => {
                const newFiles = [...files];
                try {
                  if (editFileDialog!.info.timestamp) {
                    editFileDialog!.info.timestamp = new Date(
                      editFileDialog!.info.timestamp,
                    ).toISOString();
                    newFiles[editFileDialog!.index] = editFileDialog!.info;
                    setFiles(newFiles);
                  }
                  setEditFileDialog(undefined);
                } catch (e) {
                  toast.error("Invalid timestamp!");
                }
              }}
            >
              Confirm
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog> */}
    </div>
  );
}
 