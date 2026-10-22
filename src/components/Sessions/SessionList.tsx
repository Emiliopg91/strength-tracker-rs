import { AppContext } from "@/context/AppContext";
import { useBackendEvent } from "@/hooks/useBackendEvent";
import { useBackendList } from "@/hooks/useBackendList";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BackendClient } from "@/utils/backend/client";
import { SessionListItem } from "@/utils/backend/models";
import { useContext, useState } from "react";
import { SessionModal } from "./SessionModal";
import { SessionRow } from "./helpers/SessionRow";
import { BackendListener } from "@/utils/backend/listener";
import { SessionFrontDetails, SessionUtils } from "@/utils/SessionUtils";
import "@/styles/Sessions/SessionList.css";
import { Button } from "@mui/material";
import { SessionsCompareModal } from "./SessionsCompareModal";
import { useFilterList } from "@/hooks/useFilterList";

export function SessionsList() {
    const { sessionsVersion } = useContext(AppContext);
    const withLoading = useLoadingTask();
    const { translate, fromKg, formatTimeDate } =
        useContext(I18nSettingsContext);
    const [sessions, setSessions, refreshList] =
        useBackendList<SessionListItem>(
            () => BackendClient.getSessions(null),
            [sessionsVersion],
        );
    const [sessionDetails, setSessionDetails] = useState<
        SessionFrontDetails | undefined
    >(undefined);
    const [toCompare, setToCompare] = useState<number[]>([]);
    const [toCompareName, setToCompareName] = useState<string | undefined>(
        undefined,
    );
    const [comparingSessions, setComparingSessions] = useState<
        [SessionFrontDetails, SessionFrontDetails] | undefined
    >(undefined);

    useBackendEvent(BackendListener.onSessionLocationUpdate, (data) => {
        setSessions((prev) => {
            return prev.map((item) => {
                if (item.timestamp !== data.session) return item;
                return { ...item, name: data.location };
            });
        });
    });

    const getSessionDetails = (timestamp: number) => {
        withLoading(BackendClient.getSessionDetails(timestamp)).then(
            (details) => {
                setSessionDetails(
                    SessionUtils.detailsFromBackend(details, fromKg),
                );
            },
        );
    };

    const toggleSelect = (session: SessionListItem) => {
        if (
            (session.has_sets && toCompareName === undefined) ||
            toCompareName == session.name
        ) {
            const was0 = toCompare.length == 0;
            if (toCompare.includes(session.timestamp)) {
                const toComp = [...toCompare].filter(
                    (date) => date != session.timestamp,
                );
                if (toComp.length == 0) {
                    setToCompareName(undefined);
                }
                setToCompare(toComp);
            } else {
                if (toCompare.length < 2) {
                    setToCompare([...toCompare, session.timestamp]);
                    if (was0) {
                        setToCompareName(session.name);
                    }
                }
            }
        }
    };

    const goToCompare = () => {
        withLoading(
            Promise.all(
                toCompare
                    .toSorted((a, b) => a - b)
                    .map((d) => BackendClient.getSessionDetails(d)),
            ),
        ).then(([first, second]) => {
            setComparingSessions([
                SessionUtils.detailsFromBackend(first, fromKg),
                SessionUtils.detailsFromBackend(second, fromKg),
            ]);
        });
    };

    const [filter, filteredSessions] = useFilterList(sessions, (session) => {
        return [
            formatTimeDate(session.timestamp),
            session.name,
            translate("sport_" + session.sport) +
                " - " +
                translate("sport_" + session.sport + "_" + session.sub_sport),
        ];
    });

    return (
        <>
            {filter}
            <div id="list-layer">
                <table>
                    <thead>
                        <tr>
                            <th></th>
                            <th className="text-center">{translate("date")}</th>
                            <th className="text-center">
                                {translate("sport")}
                            </th>
                            <th className="text-center">{translate("name")}</th>
                            <th className="text-center">
                                {translate("active_calories")}
                            </th>
                            <th className="text-center">
                                {translate("workout_load")}
                            </th>
                        </tr>
                    </thead>

                    <tbody>
                        {filteredSessions.map((session) => (
                            <SessionRow
                                key={session.timestamp}
                                session={session}
                                onSelect={getSessionDetails}
                                selectedForCompare={toCompare.includes(
                                    session.timestamp,
                                )}
                                onToggleCompare={toggleSelect}
                            />
                        ))}
                    </tbody>
                </table>
            </div>

            <div>
                {sessionDetails && (
                    <SessionModal
                        session={sessionDetails}
                        onClose={() => setSessionDetails(undefined)}
                        onUpdate={() => refreshList()}
                    />
                )}
            </div>

            {toCompare.length == 2 && (
                <div className="list-action-bar list-action-bar-row">
                    <Button
                        id="add-measure-button"
                        variant="contained"
                        className="full-width-button"
                        onClick={goToCompare}
                    >
                        {translate("compare")}
                    </Button>
                </div>
            )}
            {comparingSessions && (
                <SessionsCompareModal
                    sessions={comparingSessions}
                    onClose={() => setComparingSessions(undefined)}
                />
            )}
        </>
    );
}
