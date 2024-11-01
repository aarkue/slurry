import "@/globals.css";
import { Toaster } from "react-hot-toast";
import { AppContext, AppContextType } from "./AppContext";
import ConnectionConfigForm from "./components/ConnectionConfigForm";
import OCELExtractor from "./components/OCELExtractor";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";

export default function App({ context }: { context: AppContextType }) {
  return (
    <AppContext.Provider value={context}>
      <main>
        <Toaster position="top-right" />
        <Tabs className="mt-2" defaultValue="data-collection">
          <div className="text-center">
            <TabsList>
              <TabsTrigger value="data-collection" className="font-semibold">
                Data Collection
              </TabsTrigger>
              <TabsTrigger value="ocel-extraction" className="font-semibold">
                OCEL Extraction
              </TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="data-collection">
            <ConnectionConfigForm />
          </TabsContent>

          <TabsContent value="ocel-extraction">
            <OCELExtractor />
          </TabsContent>
        </Tabs>
      </main>
    </AppContext.Provider>
  );
}
