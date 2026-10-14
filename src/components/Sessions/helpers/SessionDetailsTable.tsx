import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionFrontDetails } from "@/utils/SessionUtils";
import { useContext } from "react";

type Props = {
  session: SessionFrontDetails;
};

export function SessionDetailsTable({ session }: Props) {
  const {
    translate,
    toKm,
    fromKm,
    fromKg,
    getDistanceUnit,
    getWeightUnit,
    formatDuration,
    formatTimeDate,
    formatNumber,
  } = useContext(I18nSettingsContext);

  return (
    <table id="session-details-table">
      <colgroup>
        <col className="col-200" />
        <col className="col-250" />
        <col />
      </colgroup>
      <tbody>
        <tr>
          <td>{translate("date")}:</td>
          <td>{formatTimeDate(session.timestamp)}</td>
        </tr>
        <tr>
          <td>{translate("time")}:</td>
          <td>
            {(session.active_time > 0
              ? formatDuration(session.active_time) + " / "
              : "") + formatDuration(session.total_elapsed_time)}
          </td>
        </tr>
        <tr>
          <td>{translate("calories")}:</td>
          <td>{formatNumber(session.active_calories, 0)} Kcal</td>
        </tr>
        {session.distance != null && session.distance > 0 && (
          <>
            <tr>
              <td>{translate("distance")}:</td>
              <td>
                {formatNumber(fromKm(session.distance), 2)} {getDistanceUnit()}
              </td>
            </tr>
            <tr>
              <td>{translate("speed")}:</td>
              <td>
                {formatNumber(
                  fromKm(
                    session.distance / (session.total_elapsed_time / 3600),
                  ),
                  2,
                )}{" "}
                {getDistanceUnit()}/h (
                {formatDuration(
                  toKm(session.total_elapsed_time / session.distance),
                )}{" "}
                min/{getDistanceUnit()}){" "}
              </td>
            </tr>
          </>
        )}
        {session.elevations && (
          <tr>
            <td>{translate("elevations")}:</td>
            <td>
              {formatNumber(session.elevations[0], 0) +
                " ↑   ↓ " +
                formatNumber(session.elevations[1], 0)}
            </td>
          </tr>
        )}
        {session.volume > 0 && (
          <tr>
            <td>{translate("volume")}:</td>
            <td>
              {formatNumber(fromKg(session.volume), 1)} {getWeightUnit()}
            </td>
          </tr>
        )}
        <tr>
          <td>{translate("workout_load")}:</td>
          <td>{formatNumber(session.training_load, 0)}</td>
        </tr>
        {session.device && (
          <tr>
            <td>{translate("imported_from")}:</td>
            <td>{session.device}</td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
