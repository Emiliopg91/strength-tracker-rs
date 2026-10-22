import { JSX, useContext } from "react";
import PageContent from "./PageContent";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ReportData } from "@/utils/backend/models";

type Props = {
    data: ReportData;
};

export default function ContentTable({ data }: Props): JSX.Element {
    const { translate, formatTimeDate } = useContext(I18nSettingsContext);
    return (
        <PageContent>
            <h2>{translate("content_table")}</h2>
            <ol>
                <li>{translate("notes_section")}</li>
                {data.body_metrics.length > 0 && (
                    <li>{translate("summary")}</li>
                )}
                {data.body_metrics.length > 0 && (
                    <li>{translate("body_composition")}</li>
                )}
                {data.sessions.length > 0 && (
                    <li>
                        {translate("sessions")}
                        <ul>
                            {data.sessions.map((session) => (
                                <li key={session.timestamp}>
                                    {session.name +
                                        " @ " +
                                        formatTimeDate(session.timestamp)}
                                </li>
                            ))}
                        </ul>
                    </li>
                )}
                {data.exercises.length > 0 && (
                    <li>
                        {translate("exercises")}
                        <ul>
                            {data.exercises.map((exercise) => (
                                <li key={exercise.category + "_" + exercise.id}>
                                    {translate(
                                        "exercise_" +
                                            exercise.category +
                                            "_" +
                                            exercise.id,
                                    )}
                                </li>
                            ))}
                        </ul>
                    </li>
                )}
            </ol>
        </PageContent>
    );
}
