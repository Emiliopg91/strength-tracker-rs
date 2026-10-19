import { JSX, useContext } from "react";
import PageContent from "./PageContent";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";

type Props = {
    beginDate: number;
    endDate: number;
};

export default function Cover({ beginDate, endDate }: Props): JSX.Element {
    const { translate, formatDate } = useContext(I18nSettingsContext);

    const begin = formatDate(beginDate);
    const end = formatDate(endDate);

    return (
        <PageContent>
            <div className="report-cover">
                <h1>{translate("progression_report")}</h1>
                <span>{begin + " - " + end}</span>
            </div>
        </PageContent>
    );
}
