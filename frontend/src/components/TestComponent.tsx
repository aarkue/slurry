import toast from "react-hot-toast"
import {RxCheckCircled} from "react-icons/rx"

export default function TestComponent() {
    return <div className="w-fit h-fit p-2 rounded-lg m-2 bg-emerald-100 flex items-center gap-x-2">
        <p className="text-3xl font-black text-purple-800">This is the test component working correctly :)</p>
        <RxCheckCircled className="size-8 text-green-500"/>
        <button className="bg-red-400 p-1 rounded " onClick={() => {
            toast.success("Yippieh!");
        }}>
            Toast
        </button>
    </div>
}