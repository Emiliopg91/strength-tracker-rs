import { JSX, useContext } from "react";
import PageContent from "./PageContent";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ReportData } from "@/utils/backend/models";

type Props = {
    data: ReportData;
};

export default function SummarySection({ data }: Props): JSX.Element {
    const { translate } = useContext(I18nSettingsContext);

    return (
        <PageContent>
            <h2>{translate("summary")}</h2>
            <div className="section-container">
                <b>{translate("body_metric_entries") + ": "}</b>
                {data.body_metrics.length}
                <br />
                <b>{translate("sessions") + ": "}</b>
                {data.sessions.length}
                <br />
                <b>{translate("exercises") + ": "}</b>
                {data.exercises.length}
                <br />
            </div>
        </PageContent>
    );
}
