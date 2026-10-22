import { JSX, useContext, useEffect, useState } from "react";
import Cover from "./helpers/Cover";
import BodySection from "./helpers/BodySection";
import ExercisesSection from "./helpers/ExercisesSection";
import { AppContext } from "@/context/AppContext";
import { BackendClient } from "@/utils/backend/client";
import { ReportData } from "@/utils/backend/models";
import ContentTable from "./helpers/ContentTable";
import SessionsSection from "./helpers/SessionsSection";
import NotesSection from "./helpers/NotesSection";

type Props = {
    beginDate: number;
    endDate: number;
};

export default function Report({ beginDate, endDate }: Props): JSX.Element {
    const { appReady } = useContext(AppContext);
    const [data, setData] = useState<ReportData>();

    useEffect(() => {
        if (!appReady) return;
        // Everything the sections need, fetched at once so the report renders in a single pass
        BackendClient.getReportData(beginDate, endDate).then(setData);
    }, [appReady, beginDate, endDate]);

    // Children's effects run before this one, so every section is already in the DOM.
    // Sections must stay pure (no fetching of their own) for this to hold
    useEffect(() => {
        if (!data) return;
        (async () => {
            // The window is never shown, so no frames are painted (rAF and ResizeObserver
            // stay suspended): printing lays out the DOM on its own, only fonts must be loaded
            await document.fonts.ready;
            await BackendClient.reportReady();
        })();
    }, [data]);

    if (!data) {
        return <></>;
    }

    return (
        <>
            <Cover beginDate={beginDate} endDate={endDate} />
            <ContentTable data={data} />
            <NotesSection />
            <BodySection bodyMetrics={data.body_metrics} />
            <SessionsSection
                sessions={data.sessions}
                exercises={data.exercises}
            />
            <ExercisesSection exercises={data.exercises} />
        </>
    );
}
