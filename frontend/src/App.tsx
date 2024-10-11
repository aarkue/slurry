
import { Toaster } from "react-hot-toast";
import "@/globals.css";
import ConnectionConfigForm from "./components/ConnectionConfigForm";
import { AppContext, AppContextType } from "./AppContext";

export default function App({ context }: { context: AppContextType }) {
    return <AppContext.Provider value={context}>
        <main>
            <Toaster position="top-right" />
            <ConnectionConfigForm />
        </main>
    </AppContext.Provider>
}