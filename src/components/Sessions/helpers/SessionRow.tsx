import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionListItem } from "@/utils/backend/models";
import { useContext } from "react";
import EmojiEventsIcon from "@mui/icons-material/EmojiEvents";

type Props = {
    session: SessionListItem;
    onSelect: (timestamp: number) => void;
    onToggleCompare: (session: SessionListItem) => void;
    selectedForCompare: boolean;
};

export function SessionRow({
    session,
    selectedForCompare,
    onSelect,
    onToggleCompare,
}: Props) {
    const { translate, formatTimeDate, formatNumber } =
        useContext(I18nSettingsContext);

    return (
        <tr
            onClick={() => onSelect(session.timestamp)}
            onContextMenu={(e) => {
                e.preventDefault();
                onToggleCompare(session);
            }}
            className={`clickable-row ${selectedForCompare ? "row-compare-selected" : ""}`}
        >
            <td>
                {session.has_record && (
                    <EmojiEventsIcon fontSize="small" className="trophy-icon" />
                )}
            </td>
            <td>{formatTimeDate(session.timestamp)}</td>
            <td>
                {translate("sport_" + session.sport) +
                    " - " +
                    translate(
                        "sport_" + session.sport + "_" + session.sub_sport,
                    )}
            </td>
            <td>{session.name}</td>
            <td>{formatNumber(session.active_calories, 0)}</td>
            <td>{formatNumber(session.training_load, 0)}</td>
        </tr>
    );
}
