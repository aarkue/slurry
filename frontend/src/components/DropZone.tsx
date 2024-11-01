import toast from "react-hot-toast";

const VALID_OCEL_MIME_TYPES = ["application/json"];
const ACCEPTED_TYPES = [".json"];

export default function DropZone({
  onFilesAdded,
}: {
  onFilesAdded: (files: File[]) => unknown;
}) {
  return (
    <div
      className="flex items-center justify-center w-full max-w-2xl mx-auto"
      onDragOver={(ev) => {
        ev.preventDefault();
        const items = ev.dataTransfer.items;
        const addedFiles = [...items]
          .map((f) => f.getAsFile())
          .filter(Boolean) as File[];
        const incorrectMimeFiles = addedFiles.filter(
          (f) => !VALID_OCEL_MIME_TYPES.includes(f?.type ?? ""),
        );
        if (incorrectMimeFiles.length > 0) {
          toast(
            `${incorrectMimeFiles.length} Files of type ${incorrectMimeFiles
              .map((f) => f?.type)
              .join(
                ", ",
              )} are not supported!\n\nIf you are sure that this is an valid file, please select it manually by clicking on the dropzone.`,
            { id: "unsupported-file" },
          );
        }
      }}
      onDrop={(ev) => {
        ev.preventDefault();
        const files = ev.dataTransfer.items;
        if (files.length > 0) {
          const addedFiles = [...files]
            .map((f) => f.getAsFile())
            .filter(Boolean) as File[];
          const correctMimeFiles = addedFiles.filter((f) =>
            VALID_OCEL_MIME_TYPES.includes(f?.type ?? ""),
          );
          const incorrectMimeFiles = addedFiles.filter(
            (f) => !VALID_OCEL_MIME_TYPES.includes(f?.type ?? ""),
          );
          if (incorrectMimeFiles.length > 0) {
            toast(
              `${incorrectMimeFiles.length} Files of type ${incorrectMimeFiles
                .map((f) => f?.type)
                .join(
                  ", ",
                )} are not supported!\n\nIf you are sure that this is an valid file, please select it manually by clicking on the dropzone.`,
              { id: "unsupported-file" },
            );
          }
          if (correctMimeFiles.length > 0) {
            onFilesAdded(correctMimeFiles);
          }
        }
      }}
    >
      <label
        htmlFor="dropzone-file"
        className="flex flex-col items-center justify-center w-full h-48 border-2 border-gray-400 border-dashed rounded-lg cursor-pointer bg-blue-50/20 hover:bg-blue-100/30"
      >
        <div className="flex flex-col items-center justify-center pt-5 pb-6">
          <p className="mb-2 text-sm text-gray-500">
            <span className="font-semibold">Click to select an file</span> or
            drag a file here
          </p>
          <p className="text-xs text-gray-500">
            Supported: {ACCEPTED_TYPES.join(", ")}
          </p>
        </div>
        <input
          onChange={(ev) => {
            if (
              ev.currentTarget.files !== null &&
              ev.currentTarget.files.length > 0
            ) {
              onFilesAdded([...ev.currentTarget.files]);
            }
          }}
          id="dropzone-file"
          type="file"
          className="hidden"
          accept={ACCEPTED_TYPES.join(", ")}
        />
      </label>
    </div>
  );
}
