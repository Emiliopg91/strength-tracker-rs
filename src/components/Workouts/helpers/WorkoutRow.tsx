import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { WorkoutListItem } from "@/utils/backend/models";
import { useContext } from "react";
import { WorkoutActionsMenu } from "./WorkoutActionsMenu";

type Props = {
  workout: WorkoutListItem;
  onSelect: (name: string) => void;
  onOpenEdit: (name: string) => void;
  onOpenDuplicate: (name: string) => void;
};

export function WorkoutRow({
  workout,
  onSelect,
  onOpenEdit,
  onOpenDuplicate,
}: Props) {
  const { translate, formatDuration, formatTimeDate, formatNumber } =
    useContext(I18nSettingsContext);

  return (
    <tr
      className={"clickable-row" + (workout.enabled ? "" : " workout-disabled")}
      onClick={() => onSelect(workout.name)}
    >
      <td className="text-left">
        {workout.name.length > 0 && <span>{workout.name}</span>}
        {workout.name.length == 0 && <span>{translate("other")}</span>}
      </td>
      <td>
        {workout.latest_session && formatTimeDate(workout.latest_session)}
      </td>
      <td>{formatNumber(workout.sessions, 0)}</td>
      <td>{workout.avg_time > 0 && formatDuration(workout.avg_time)}</td>
      <td>{workout.avg_load > 0 && workout.avg_load}</td>
      {/* Menu is portaled but React events still bubble to the row */}
      <td onClick={(e) => e.stopPropagation()}>
        {workout.has_steps && (
          <WorkoutActionsMenu
            workoutName={workout.name}
            onOpenEdit={() => onOpenEdit(workout.name)}
            onOpenDuplicate={() => onOpenDuplicate(workout.name)}
            dense
          />
        )}
      </td>
    </tr>
  );
}
