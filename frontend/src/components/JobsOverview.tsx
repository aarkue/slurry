import { AppContext } from '@/AppContext';
import { ResponsiveLine } from '@nivo/line'
import { useCallback, useContext, useEffect, useMemo, useState } from 'react'


export default function JobsOverview() {
    const [data, setData] = useState<{ time: Date, counts: Record<string, number> }[]>([]);
    const { getSqueue } = useContext(AppContext);
    const updateData = useCallback(() => getSqueue().then(([time, rows]) => {
        const counts: Record<string, number> = {};
        for (const row of rows) {
            if (!(row.state in counts)) {
                counts[row.state] = 0;
            }
            counts[row.state] += 1;
        }
        console.log(counts)
        setData((prevData) => [...prevData, { time: new Date(time), counts }])
    }),[]);
    useEffect(() => {
        updateData()
        const t = setInterval(() => {
            updateData()
        }, 10 * 1000);
        return () => {
            clearInterval(t);
        }
    }, [])
    return <div className='h-[20rem] w-11/12 mx-auto'>
        {data.length > 0 && <MyResponsiveLine data={[...(new Set(["PENDING","RUNNING","COMPLETING"]).union(new Set(data.flatMap((d) => Object.keys(d.counts))))).values()].map((state) => ({
            id: state,
            color: state == "RUNNING" ? "#7FE575" : state === "PENDING" ? "#42C7D5" :  state === "COMPLETING" ? "#EAC5D8" : "red",
            data: data.filter((_,i) => i === data.length-1 || i%Math.max(1,Math.floor(data.length/30)) === 0).map((i) => ({ x: i.time, y: i.counts[state] ?? 0 }))
        }))} />}
    </div>
}


const MyResponsiveLine = ({ data /* see data tab */ }: {
    data: Array<{
        id: string | number,
        color?: string,
        data: Array<{
            x: number | string | Date
            y: number | string | Date
        }>
    }>
}) => (
    <ResponsiveLine
        data={data}
        margin={{ top: 50, right: 110, bottom: 50, left: 60 }}
        xScale={{ type: 'time', nice: false, useUTC: false, format: '%H:%M:%S'}}
        yScale={{
            type: 'linear',
            min: 0,
            max: 'auto',
            stacked: true,
            reverse: false,
        }}
        yFormat=" >-.0f"
        xFormat={(v) => new Date(v).toLocaleTimeString()}
        axisTop={null}
        axisRight={null}
        axisBottom={{
            // tickSize: 5,
            // tickPadding: 5,
            // tickRotation: 0,
            legend: 'Time',
            legendOffset: 36,
            legendPosition: 'middle',
            // truncateTickAt: 0,
            format: '%H:%M:%S', //  %d.%m
            // renderTick: (x) => <span>{x.textX}</span>
        }}
        axisLeft={{
            tickSize: 5,
            tickPadding: 5,
            tickRotation: 0,
            legend: 'Count',
            legendOffset: -45,
            legendPosition: 'middle',
            truncateTickAt: 0,
        }}
        // pointSize={10}
        pointBorderWidth={2}
        // pointLabel="data.yFormatted"
        pointLabelYOffset={-12}
        enableArea={true}
        areaOpacity={0.3}
        // enableTouchCrosshair={true}
        // useMesh={true}
        enableSlices={'x'}
        // layers={['markers', 'lines', 'areas', 'axes', 'crosshair', 'legends', 'grid', 'mesh', 'points']}
        colors={data.map(d => d.color ?? "black")}
        legends={[
            {
                anchor: 'bottom-right',
                direction: 'column',
                justify: false,
                translateX: 100,
                translateY: 0,
                itemsSpacing: 0,
                itemDirection: 'left-to-right',
                itemWidth: 80,
                itemHeight: 20,
                itemOpacity: 0.75,
                symbolSize: 12,
                symbolShape: 'circle',
                symbolBorderColor: 'rgba(0, 0, 0, .5)',
                effects: [
                    {
                        on: 'hover',
                        style: {
                            itemBackground: 'rgba(0, 0, 0, .03)',
                            itemOpacity: 1
                        }
                    }
                ]
            }
        ]}
    />
)